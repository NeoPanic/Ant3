//! The ACP v1 stdio exchange. Ant³ advertises no client filesystem or terminal APIs.
use super::Executor;
use ant3::CancellationToken;
use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

const MAX_FRAME: u64 = 1024 * 1024;
const MAX_OUTPUT: usize = 32_000;
type ProtocolResult<T> = Result<T, &'static str>;

pub(super) struct Session {
    #[cfg(unix)]
    _group: crate::local::ProcessGroup,
    _child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    frame: Vec<u8>,
    next_id: u64,
    id: String,
}

pub(super) struct Reply {
    pub text: String,
    pub truncated: bool,
    pub reason: String,
}

pub(super) enum TurnError {
    BeforePrompt(&'static str),
    Unknown(&'static str),
    Cancelled,
}

impl Session {
    pub async fn open(executor: &Executor, workspace: &Path) -> ProtocolResult<Self> {
        let mut command = Command::new(&executor.command);
        command
            .args(&executor.args)
            .current_dir(workspace)
            .env_clear()
            .envs(
                ["PATH", "HOME", "USER", "TMPDIR", "LANG"]
                    .into_iter()
                    .chain(executor.inherit_env.iter().map(String::as_str))
                    .filter_map(|name| std::env::var_os(name).map(|value| (name, value))),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|_| "acp_spawn_failed")?;
        #[cfg(unix)]
        let group = crate::local::ProcessGroup(child.id().ok_or("acp_spawn_failed")? as i32);
        let mut session = Self {
            #[cfg(unix)]
            _group: group,
            input: child.stdin.take().ok_or("acp_pipe_closed")?,
            output: BufReader::new(child.stdout.take().ok_or("acp_pipe_closed")?),
            _child: child,
            frame: Vec::new(),
            next_id: 0,
            id: String::new(),
        };
        let initialized = session
            .request(
                "initialize",
                json!({
                    "protocolVersion":1,"clientCapabilities":{},
                    "clientInfo":{"name":"ant3","version":env!("CARGO_PKG_VERSION")}
                }),
            )
            .await?;
        if initialized["protocolVersion"] != 1 {
            return Err("acp_unsupported_protocol_version");
        }
        if let Some(method) = &executor.auth_method {
            if !initialized["authMethods"]
                .as_array()
                .is_some_and(|methods| methods.iter().any(|entry| entry["id"] == method.as_str()))
            {
                return Err("acp_auth_method_unavailable");
            }
            session
                .request("authenticate", json!({"methodId":method}))
                .await
                .map_err(|_| "acp_auth_failed")?;
        }
        let created = session
            .request("session/new", json!({"cwd":workspace,"mcpServers":[]}))
            .await?;
        session.id = created["sessionId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 1024)
            .ok_or("acp_invalid_session_id")?
            .into();
        Ok(session)
    }

    pub async fn turn(
        &mut self,
        prompt: &str,
        cancel: CancellationToken,
        timeout: Duration,
        mut progress: impl FnMut(&Value) -> ProtocolResult<()>,
    ) -> Result<Reply, TurnError> {
        if cancel.is_cancelled() {
            return Err(TurnError::BeforePrompt("cancelled"));
        }
        let deadline = tokio::time::Instant::now() + timeout;
        let send = self.start(
            "session/prompt",
            json!({
                "sessionId":self.id,"prompt":[{"type":"text","text":prompt}]
            }),
        );
        let id = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TurnError::Unknown("cancelled_during_acp_send")),
            result = tokio::time::timeout_at(deadline, send) => {
                result.map_err(|_| TurnError::Unknown("acp_send_timeout"))?.map_err(TurnError::Unknown)?
            }
        };

        let mut reply = Reply {
            text: String::new(),
            truncated: false,
            reason: String::new(),
        };
        let mut collect = |update: &Value| {
            if !reply.truncated
                && update["sessionUpdate"] == "agent_message_chunk"
                && update["content"]["type"] == "text"
                && let Some(text) = update["content"]["text"].as_str()
            {
                let remaining = MAX_OUTPUT - reply.text.len();
                let end = text.floor_char_boundary(remaining.min(text.len()));
                reply.text.push_str(&text[..end]);
                reply.truncated |= end < text.len();
            }
            progress(update)
        };
        let finished = tokio::select! {
            biased;
            result = tokio::time::timeout_at(deadline, self.response(id, false, &mut collect)) => match result {
                Ok(receipt) => Some(receipt.map_err(TurnError::Unknown)?),
                Err(_) => None,
            },
            _ = cancel.cancelled() => None,
        };
        let receipt = if let Some(receipt) = finished {
            receipt
        } else {
            let cancelled = tokio::time::timeout(Duration::from_secs(2), async {
                self.send(json!({
                    "jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":self.id}
                }))
                .await?;
                self.response(id, true, &mut collect).await
            })
            .await;
            match cancelled {
                Ok(Ok(receipt)) if receipt["stopReason"] == "cancelled" => {
                    return Err(TurnError::Cancelled);
                }
                Ok(Ok(receipt)) => receipt,
                _ => return Err(TurnError::Unknown("acp_cancel_unconfirmed")),
            }
        };
        reply.reason = receipt["stopReason"]
            .as_str()
            .filter(|reason| {
                matches!(
                    *reason,
                    "end_turn" | "max_tokens" | "max_turn_requests" | "refusal" | "cancelled"
                )
            })
            .ok_or(TurnError::Unknown("acp_invalid_stop_reason"))?
            .into();
        if reply.reason == "cancelled" {
            return Err(TurnError::Cancelled);
        }
        Ok(reply)
    }

    async fn request(&mut self, method: &str, params: Value) -> ProtocolResult<Value> {
        let id = self.start(method, params).await?;
        self.response(id, false, |_| Ok(())).await
    }

    async fn start(&mut self, method: &str, params: Value) -> ProtocolResult<u64> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        Ok(id)
    }

    async fn send(&mut self, request: Value) -> ProtocolResult<()> {
        let mut line = serde_json::to_vec(&request).map_err(|_| "acp_protocol_error")?;
        line.push(b'\n');
        self.input
            .write_all(&line)
            .await
            .map_err(|_| "acp_pipe_closed")
    }

    async fn response(
        &mut self,
        id: u64,
        cancelling: bool,
        mut update: impl FnMut(&Value) -> ProtocolResult<()>,
    ) -> ProtocolResult<Value> {
        loop {
            let n = (&mut self.output)
                .take(MAX_FRAME + 1 - self.frame.len() as u64)
                .read_until(b'\n', &mut self.frame)
                .await
                .map_err(|_| "acp_pipe_closed")?;
            if n == 0 {
                return Err("acp_disconnected");
            }
            if self.frame.len() as u64 > MAX_FRAME {
                return Err("acp_frame_too_large");
            }
            let message: Value =
                serde_json::from_slice(&self.frame).map_err(|_| "acp_protocol_error")?;
            self.frame.clear();
            if message["jsonrpc"] != "2.0" {
                return Err("acp_protocol_error");
            }
            if let Some(method) = message["method"].as_str() {
                let params = &message["params"];
                if let Some(request_id) = message.get("id") {
                    self.respond_to_client_request(request_id, method, params, cancelling)
                        .await?;
                } else if method == "session/update" && params["sessionId"] == self.id.as_str() {
                    update(&params["update"])?;
                }
                continue;
            }
            if message["id"] != id
                || message.get("result").is_some() == message.get("error").is_some()
            {
                return Err("acp_protocol_error");
            }
            if message.get("error").is_some() {
                return Err("acp_remote_error");
            }
            return Ok(message["result"].clone());
        }
    }

    async fn respond_to_client_request(
        &mut self,
        request_id: &Value,
        method: &str,
        params: &Value,
        cancelling: bool,
    ) -> ProtocolResult<()> {
        let mut response = json!({"jsonrpc":"2.0","id":request_id});
        if method == "session/request_permission" && params["sessionId"] == self.id.as_str() {
            let reject = params["options"].as_array().and_then(|options| {
                options
                    .iter()
                    .find(|option| option["kind"] == "reject_once")
                    .and_then(|option| option["optionId"].as_str())
            });
            response["result"] = match reject.filter(|_| !cancelling) {
                Some(id) => json!({"outcome":{"outcome":"selected","optionId":id}}),
                None => json!({"outcome":{"outcome":"cancelled"}}),
            };
        } else {
            response["error"] = json!({"code":-32601,"message":"Client method unavailable"});
        }
        self.send(response).await
    }
}
