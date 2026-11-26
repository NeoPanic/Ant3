//! Run-scoped system plugin for operator-registered ACP executors.
mod transport;

use crate::Mutex;
use ant3::{CancellationToken, Context, Error, Result, Tool, ToolOutput, ToolSpec};
use ant3_platform::store::Store;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use transport::Session;

const MAX_SESSIONS: usize = 8;
const MAX_PROMPT: usize = 32_000;
const TURN_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Default)]
pub struct AcpExecutors(Arc<BTreeMap<String, Executor>>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Executor {
    id: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    inherit_env: Vec<String>,
    auth_method: Option<String>,
}

impl AcpExecutors {
    pub fn load(
        path: &Path,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::parse(&std::fs::read(path)?).map_err(Into::into)
    }

    fn parse(bytes: &[u8]) -> std::result::Result<Self, &'static str> {
        let entries: Vec<Executor> =
            serde_json::from_slice(bytes).map_err(|_| "invalid ACP executor configuration")?;
        let mut executors = BTreeMap::new();
        for entry in entries {
            if !valid_id(&entry.id)
                || entry.command.trim().is_empty()
                || entry.command.contains('\0')
                || entry.args.iter().any(|arg| arg.contains('\0'))
                || entry.inherit_env.iter().any(|name| {
                    name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                })
                || entry
                    .auth_method
                    .as_ref()
                    .is_some_and(|id| id.trim().is_empty())
            {
                return Err("invalid ACP executor registration");
            }
            if executors.insert(entry.id.clone(), entry).is_some() {
                return Err("duplicate ACP executor id");
            }
        }
        Ok(Self(Arc::new(executors)))
    }

    pub(super) fn capability(&self) -> Option<ToolSpec> {
        (!self.0.is_empty()).then(|| self.tool_spec())
    }

    fn tool_spec(&self) -> ToolSpec {
        let executors: Vec<_> = self.0.keys().map(String::as_str).collect();
        ToolSpec {
            name: "external.acp".into(),
            description: format!(
                "Send work to a registered ACP executor in this workspace. Executors: {}. \
                 Omit session_id to start; reuse the returned session_id with the same executor \
                 to send a follow-up during this Run. Review the returned text and stop reason.",
                executors.join(", ")
            ),
            parameters: json!({
                "type":"object",
                "additionalProperties":false,
                "required":["executor","prompt"],
                "properties":{
                    "executor":{"type":"string","enum":executors},
                    "prompt":{"type":"string","minLength":1,"maxLength":MAX_PROMPT},
                    "session_id":{"type":"string","description":"Handle returned earlier in this Run"}
                }
            }),
            mutates: true,
        }
    }

    pub(super) fn tool(&self, store: Store, workspace: PathBuf) -> Option<Arc<dyn Tool>> {
        (!self.0.is_empty()).then(|| {
            Arc::new(AcpTool {
                executors: self.clone(),
                store,
                workspace,
                sessions: Mutex::default(),
            }) as Arc<dyn Tool>
        })
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

struct AcpTool {
    executors: AcpExecutors,
    store: Store,
    workspace: PathBuf,
    sessions: Mutex<HashMap<String, (String, Session)>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prompt {
    executor: String,
    prompt: String,
    session_id: Option<String>,
}

#[async_trait::async_trait]
impl Tool for AcpTool {
    fn spec(&self) -> ToolSpec {
        self.executors.tool_spec()
    }

    async fn execute(
        &self,
        context: &Context,
        args: Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let input: Prompt =
            serde_json::from_value(args).map_err(|_| Error::new("invalid_acp_input"))?;
        if input.prompt.trim().is_empty() || input.prompt.len() > MAX_PROMPT {
            return Err(Error::new("invalid_acp_prompt"));
        }
        let executor = self
            .executors
            .0
            .get(&input.executor)
            .ok_or_else(|| Error::new("acp_executor_not_registered"))?;
        let mut sessions = self.sessions.lock().await;
        if cancel.is_cancelled() {
            return Err(Error::new("cancelled"));
        }
        let (handle, mut session) = if let Some(handle) = input.session_id {
            if !sessions
                .get(&handle)
                .is_some_and(|(id, _)| id == &input.executor)
            {
                return Err(Error::new("acp_session_not_found"));
            }
            let (_, session) = sessions.remove(&handle).unwrap();
            (handle, session)
        } else {
            if sessions.len() == MAX_SESSIONS {
                return Err(Error::new("acp_session_limit"));
            }
            std::fs::create_dir_all(&self.workspace)
                .map_err(|_| Error::new("workspace_io_error"))?;
            let workspace = self
                .workspace
                .canonicalize()
                .map_err(|_| Error::new("workspace_io_error"))?;
            let parent = self.workspace.parent().and_then(|p| p.canonicalize().ok());
            if workspace.parent() != parent.as_deref()
                || workspace.file_name() != self.workspace.file_name()
            {
                return Err(Error::new("acp_workspace_outside_instance"));
            }
            let setup = Session::open(executor, &workspace);
            let session = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(Error::new("cancelled")),
                result = tokio::time::timeout(Duration::from_secs(30), setup) => {
                    result.map_err(|_| Error::new("acp_setup_timeout"))?.map_err(Error::new)?
                }
            };
            (uuid::Uuid::new_v4().to_string(), session)
        };

        let mut progress_at = None;
        let progress = |update: &Value| {
            let kind = update["sessionUpdate"].as_str().unwrap_or_default();
            if !matches!(
                kind,
                "agent_message_chunk" | "plan" | "tool_call" | "tool_call_update"
            ) {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if progress_at
                .is_some_and(|previous| now.duration_since(previous) < Duration::from_millis(250))
            {
                return Ok(());
            }
            progress_at = Some(now);
            self.store
                .record(
                    "execution.acp_progress",
                    &context.event_id,
                    json!({
                        "executor":input.executor, "session_id":handle, "update":kind
                    }),
                )
                .map_err(|_| "acp_progress_storage_error")
        };
        let turn = session
            .turn(&input.prompt, cancel, TURN_TIMEOUT, progress)
            .await;
        match turn {
            Ok(reply) => {
                sessions.insert(handle.clone(), (input.executor.clone(), session));
                Ok(ToolOutput {
                    content: json!({
                        "executor":input.executor,"session_id":handle,
                        "output":reply.text,"output_truncated":reply.truncated,"stop_reason":reply.reason
                    }),
                    changed: true,
                })
            }
            Err(transport::TurnError::BeforePrompt(code)) => Err(Error::new(code)),
            Err(transport::TurnError::Unknown(code)) => Err(Error::new("acp_outcome_unknown")
                .explain(
                    "The execution could have changed the workspace. Inspect before retrying.",
                    json!({"executor":input.executor,"session_id":handle,"cause":code}),
                )),
            Err(transport::TurnError::Cancelled) => Err(Error::new("cancelled")),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/support/acp.rs"]
mod tests;
