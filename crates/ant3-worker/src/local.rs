//! Local tools for trusted single-user workers. Commands inherit the daemon account.
use super::*;
use std::{path::PathBuf, process::Stdio};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

const OUTPUT_LIMIT: usize = 128 * 1024;

pub(super) struct LocalTool {
    pub name: &'static str,
    pub workspace: PathBuf,
}

async fn capture(mut stream: impl AsyncRead + Unpin) -> std::io::Result<Value> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let mut total = 0usize;
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        total += n;
        let keep = n.min(OUTPUT_LIMIT - bytes.len());
        bytes.extend_from_slice(&buffer[..keep]);
    }
    Ok(json!({"text":String::from_utf8_lossy(&bytes),"truncated":total > bytes.len()}))
}

// The harness may drop the tool future on timeout, including while its pipes drain.
struct ProcessGroup(i32);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-self.0, libc::SIGKILL);
        }
    }
}

#[async_trait::async_trait]
impl Tool for LocalTool {
    fn spec(&self) -> ToolSpec {
        let (description, fields): (_, &[&str]) = match self.name {
            "bash" => (
                "Run a noninteractive Bash command in the instance workspace, including installed CLI clients. Use rg or grep, sed and tail to select relevant output. Inspect exit_code, stdout and stderr.",
                &["command"],
            ),
            "read" => (
                "Read a text file in the instance workspace. Use bash to page longer files.",
                &["path"],
            ),
            "write" => (
                "Create or replace a text file in the instance workspace.",
                &["path", "content"],
            ),
            _ => (
                "Replace exactly one occurrence of old_text in a text file. Fails if missing or ambiguous.",
                &["path", "old_text", "new_text"],
            ),
        };
        let properties: serde_json::Map<_, _> = fields
            .iter()
            .map(|k| (k.to_string(), json!({"type":"string"})))
            .collect();
        ToolSpec {
            name: self.name.into(),
            description: description.into(),
            mutates: self.name != "read",
            parameters: json!({"type":"object","properties":properties,"required":fields,"additionalProperties":false}),
        }
    }
    async fn execute(
        &self,
        _: &Context,
        args: Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let arg = |key: &str| {
            args[key]
                .as_str()
                .ok_or_else(|| Error::new("invalid_local_tool_input"))
        };
        let mut command = if self.name == "bash" {
            let mut cmd = Command::new("/bin/bash");
            cmd.args(["-c", arg("command")?]);
            cmd
        } else {
            let path = std::path::Path::new(arg("path")?);
            if path.as_os_str().is_empty()
                || path.components().any(|c| {
                    !matches!(
                        c,
                        std::path::Component::Normal(_) | std::path::Component::CurDir
                    )
                })
            {
                return Err(Error::new("invalid_workspace_path"));
            }
            let mut cmd = Command::new("python3");
            cmd.args(["-c", FILE_TOOL, self.name]).arg(path);
            match self.name {
                "write" => {
                    cmd.arg(arg("content")?);
                }
                "edit" => {
                    cmd.args([arg("old_text")?, arg("new_text")?]);
                }
                _ => {}
            }
            cmd
        };
        let io = |_| Error::new("local_tool_io_failed");
        std::fs::create_dir_all(&self.workspace).map_err(io)?;
        let mut child = command
            .current_dir(&self.workspace)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(io)?;
        let _group = ProcessGroup(child.id().unwrap() as i32);
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (status, stdout, stderr) = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::new("cancelled")),
            result = async {tokio::try_join!(child.wait(),capture(stdout),capture(stderr))} => result.map_err(io)?,
        };
        Ok(ToolOutput {
            content: json!({"exit_code":status.code(),"stdout":stdout,"stderr":stderr}),
            changed: self.name != "read" && status.success(),
        })
    }
}

const FILE_TOOL: &str = r#"
import os, sys, tempfile
from pathlib import Path
mode, filename, *args = sys.argv[1:]
path = Path(filename).resolve()
if not path.is_relative_to(Path.cwd()):
    sys.exit("Path leaves workspace")
if mode == "read":
    with path.open("rb") as source:
        import shutil
        shutil.copyfileobj(source, sys.stdout.buffer)
else:
    if mode == "edit":
        original = path.read_bytes()
        old, new = (s.encode() for s in args)
        if not old or original.count(old) != 1:
            sys.exit("Expected exactly one occurrence")
        data = original.replace(old, new, 1)
    else:
        data = args[0].encode()
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as f:
        try:
            f.write(data)
            f.flush()
            if path.exists():
                os.fchmod(f.fileno(), path.stat().st_mode)
            os.replace(f.name, path)
        finally:
            Path(f.name).unlink(missing_ok=True)
"#;
