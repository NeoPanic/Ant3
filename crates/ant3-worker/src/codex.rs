use super::*;
use std::{path::Path, process::Stdio, time::Instant};
use tokio::{io::AsyncWriteExt, process::Command};

pub(super) struct CodexTool {
    pub workspace: std::path::PathBuf,
    pub run: String,
}

#[async_trait::async_trait]
impl Tool for CodexTool {
    fn spec(&self) -> ToolSpec {
        ant3_platform::organization::codex_spec()
    }
    async fn execute(&self, _: &Context, args: Value, cancel: CancellationToken) -> Result<ToolOutput> {
        let task = args["task"].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 32000)
            .ok_or_else(|| Error::new("invalid_codex_task"))?;
        let content = execute(&self.workspace, &self.run, task.into(), cancel).await
            .map_err(|e| Error::new(e.1))?;
        Ok(ToolOutput { content, changed: true })
    }
}

async fn execute(
    workspace: &Path,
    run: &str,
    prompt: String,
    cancel: CancellationToken,
) -> PResult<Value> {
    std::fs::create_dir_all(workspace).map_err(|_| fault(500, "workspace_io_error"))?;
    let started = Instant::now();
    let operation = async {
        let mut child = Command::new("codex")
            .args([
                "exec",
                "--ephemeral",
                "--skip-git-repo-check",
                "--json",
                "--sandbox",
                "workspace-write",
                "--enable",
                "skip_host_skill_discovery",
                "-c",
                "approval_policy=\"never\"",
                "-",
            ])
            .current_dir(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| fault(503, "codex_cli_spawn_failed"))?;
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|_| fault(502, "codex_cli_input_failed"))?;
        drop(stdin);
        let result = child
            .wait_with_output()
            .await
            .map_err(|_| fault(502, "codex_cli_wait_failed"))?;
        if !result.status.success() {
            return Err(fault(502, "codex_cli_failed"));
        }
        let mut answer = None;
        let mut usage = None;
        for line in result
            .stdout
            .split(|&c| c == b'\n')
            .filter(|s| !s.is_empty())
        {
            let event: Value =
                serde_json::from_slice(line).map_err(|_| fault(502, "codex_cli_invalid_event"))?;
            match event["type"].as_str() {
                Some("item.completed") if event["item"]["type"] == "agent_message" => {
                    answer = event["item"]["text"].as_str().map(str::to_owned);
                }
                Some("turn.completed") => {
                    let u = &event["usage"];
                    usage = Some(
                        json!({"input":u["input_tokens"],"output":u["output_tokens"],"cached_input":u["cached_input_tokens"]}),
                    );
                }
                Some("turn.failed") => return Err(fault(502, "codex_cli_failed")),
                _ => {}
            }
        }
        let output = answer
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| fault(502, "codex_cli_empty_result"))?;
        Ok(
            json!({"run_id":run,"output":output,"observations":[],"usage":usage,"execution_ms":started.elapsed().as_millis()}),
        )
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(fault(409,"cancelled")),
        result = tokio::time::timeout(Duration::from_secs(600), operation) => result.unwrap_or_else(|_| Err(fault(504,"codex_cli_timeout"))),
    }
}
