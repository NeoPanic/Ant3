//! Per-instance local files and immutable, attributable shared file/JSON snapshots.
mod tools;
use crate::{
    domain::*,
    store::{Store, event},
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
pub use tools::{package, tools};
const MAX_BYTES: usize = 1_048_576;
const META: &str = "json_object('id',a.id,'instance_id',a.instance_id,'run_id',a.run_id,'kind',a.kind,'name',a.name,'path',a.path,'media_type',a.media_type,'digest',a.digest,'bytes',a.bytes,'created_at',a.created_at,'source_state',r.state)";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteInput {
    pub path: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishInput {
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
}
fn io(_: std::io::Error) -> Fault {
    fault(500, "workspace_io_error")
}
fn digest(content: &str) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(content.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
fn relative(path: &str) -> PResult<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute()
        || path.len() > 512
        || path.contains('\\')
        || p.components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(fault(400, "invalid_workspace_path"));
    }
    Ok(p.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect())
}
fn instance_root(s: &Store, instance: &str) -> PResult<PathBuf> {
    if !valid_name(instance) {
        return Err(fault(400, "invalid_instance_id"));
    }
    if !s.instance_exists(instance)? {
        return Err(fault(404, "instance_not_found"));
    }
    let root = s.workspace_root()?.join(instance);
    std::fs::create_dir_all(&root).map_err(io)?;
    Ok(root)
}
fn resolved(root: &Path, path: &str) -> PResult<PathBuf> {
    let p = relative(path)?;
    let mut current = root.to_path_buf();
    for c in p.components() {
        current.push(c);
        if let Ok(meta) = std::fs::symlink_metadata(&current)
            && meta.file_type().is_symlink()
        {
            return Err(fault(400, "workspace_symlink_not_supported"));
        }
    }
    Ok(current)
}
fn read_file(path: &Path) -> PResult<String> {
    let meta = std::fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            fault(404, "workspace_file_not_found")
        } else {
            io(e)
        }
    })?;
    if !meta.is_file() || meta.len() > MAX_BYTES as u64 {
        return Err(fault(400, "workspace_file_invalid"));
    }
    let content =
        std::fs::read_to_string(path).map_err(|_| fault(400, "workspace_utf8_required"))?;
    if content.len() > MAX_BYTES {
        return Err(fault(400, "workspace_file_invalid"));
    }
    Ok(content)
}
fn scope(tx: &Transaction<'_>, run: &str) -> PResult<String> {
    tx.query_row("SELECT i.id FROM runs r JOIN instances i ON i.id=r.instance_id WHERE r.id=? AND r.state='running' AND r.cancel_requested=0 AND i.state='busy'", [run], |r| r.get(0)).optional()?.ok_or_else(||fault(403,"workspace_run_not_authorized"))
}
fn prior(tx: &Transaction<'_>, run: &str, key: &str, fingerprint: &str) -> PResult<Option<Value>> {
    if key.is_empty() || key.len() > 160 {
        return Err(fault(400, "invalid_workspace_command_key"));
    }
    let old: Option<(String, String)> = tx
        .query_row(
            "SELECT fingerprint,response FROM workspace_commands WHERE run_id=? AND command_key=?",
            params![run, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    old.map(|(old, body)| {
        if old == fingerprint {
            Ok(serde_json::from_str(&body)?)
        } else {
            Err(fault(409, "idempotency_conflict"))
        }
    })
    .transpose()
}
fn receipt(
    tx: &Transaction<'_>,
    run: &str,
    key: &str,
    fingerprint: &str,
    value: &Value,
) -> PResult<()> {
    tx.execute(
        "INSERT INTO workspace_commands(run_id,command_key,fingerprint,response) VALUES(?,?,?,?)",
        params![run, key, fingerprint, value.to_string()],
    )?;
    Ok(())
}
impl Store {
    pub fn workspace_catalog(&self) -> PResult<Value> {
        let instances=self.transaction(|tx|{
            let mut stmt=tx.prepare("SELECT json_object('id',i.id,'preset_name',i.preset_name,'state',i.state,'artifact_count',(SELECT count(*) FROM workspace_artifacts a WHERE a.instance_id=i.id)) FROM instances i ORDER BY i.created_at DESC LIMIT 200")?;
            stmt.query_map([],|r|r.get::<_,String>(0))?.map(|r|Ok(serde_json::from_str::<Value>(&r?)?)).collect::<PResult<Vec<_>>>()
        })?;
        let configured = self.workspace_root().is_ok();
        Ok(
            json!({"configured":configured,"instances":instances,"file_limit_bytes":MAX_BYTES,"text_only":true}),
        )
    }
    pub fn workspace_files(&self, instance: &str, path: &str) -> PResult<Value> {
        let root = instance_root(self, instance)?;
        let directory = resolved(&root, path)?;
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(directory).map_err(io)?.take(201) {
            let entry = entry.map_err(io)?;
            let meta = entry.metadata().map_err(io)?;
            let symlink = entry.file_type().map_err(io)?.is_symlink();
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = entry
                .path()
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            entries.push(json!({"name":name,"path":relative,"kind":if symlink{"symlink"}else if meta.is_dir(){"directory"}else{"file"},"bytes":meta.len(),"readable":!symlink&&meta.is_file()&&meta.len()<=MAX_BYTES as u64}));
        }
        let truncated = entries.len() > 200;
        entries.truncate(200);
        entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        Ok(json!({"instance_id":instance,"path":path,"entries":entries,"truncated":truncated}))
    }
    pub fn workspace_read(&self, instance: &str, path: &str) -> PResult<Value> {
        let root = instance_root(self, instance)?;
        let content = read_file(&resolved(&root, path)?)?;
        Ok(
            json!({"instance_id":instance,"path":path,"content":content,"bytes":content.len(),"digest":digest(&content),"untrusted_content":true}),
        )
    }
    pub fn run_workspace_files(&self, run: &str, path: &str) -> PResult<Value> {
        let instance = self.transaction(|tx| scope(tx, run))?;
        self.workspace_files(&instance, path)
    }
    pub fn run_workspace_read(&self, run: &str, path: &str) -> PResult<Value> {
        let instance = self.transaction(|tx| scope(tx, run))?;
        self.workspace_read(&instance, path)
    }
    pub fn workspace_write(&self, run: &str, input: &WriteInput, key: &str) -> PResult<Value> {
        if input.path.trim().is_empty() || input.content.len() > MAX_BYTES {
            return Err(fault(400, "invalid_workspace_write"));
        }
        let fingerprint = format!("write:{}", serde_json::to_string(input)?);
        let instance = self.transaction(|tx| scope(tx, run))?;
        let root = instance_root(self, &instance)?;
        self.transaction(|tx|{
            let instance=scope(tx,run)?;if let Some(v)=prior(tx,run,key,&fingerprint)?{return Ok(v);}
            let path=resolved(&root,&input.path)?;std::fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
            // Repeating a write after a crash restores the same bytes; no append/delete command exists.
            std::fs::write(&path,input.content.as_bytes()).map_err(io)?;
            let value=json!({"instance_id":instance,"path":input.path,"bytes":input.content.len(),"digest":digest(&input.content),"saved":true});receipt(tx,run,key,&fingerprint,&value)?;
            event(tx,"workspace.file_written",&instance,json!({"run_id":run,"path":input.path,"bytes":input.content.len()}))?;Ok(value)
        })
    }
    pub fn publish_workspace_artifact(
        &self,
        run: &str,
        input: &PublishInput,
        key: &str,
    ) -> PResult<Value> {
        if input.name.trim().is_empty()
            || input.name.len() > 200
            || input.path.is_some() == input.value.is_some()
        {
            return Err(fault(400, "invalid_workspace_publication"));
        }
        let fingerprint = format!("publish:{}", serde_json::to_string(input)?);
        let instance = self.transaction(|tx| scope(tx, run))?;
        // Prior receipt wins even if the live file was subsequently edited.
        if let Some(old) = self.transaction(|tx| prior(tx, run, key, &fingerprint))? {
            return Ok(old);
        }
        let (kind, media, content) = if let Some(path) = &input.path {
            let root = instance_root(self, &instance)?;
            (
                "file",
                "text/plain; charset=utf-8",
                read_file(&resolved(&root, path)?)?,
            )
        } else {
            (
                "json",
                "application/json",
                serde_json::to_string_pretty(input.value.as_ref().unwrap())?,
            )
        };
        if content.len() > MAX_BYTES {
            return Err(fault(400, "workspace_artifact_too_large"));
        }
        self.transaction(|tx|{
            let instance=scope(tx,run)?;if let Some(old)=prior(tx,run,key,&fingerprint)?{return Ok(old);}
            let id=id("fileartifact");tx.execute("INSERT INTO workspace_artifacts(id,instance_id,run_id,kind,name,path,media_type,content,digest,bytes,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",params![id,instance,run,kind,input.name,input.path,media,content,digest(&content),content.len(),now()])?;
            let value=artifact_in(tx,&id,false)?;receipt(tx,run,key,&fingerprint,&value)?;event(tx,"workspace.artifact_published",&id,json!({"run_id":run,"instance_id":instance,"kind":kind,"name":input.name}))?;Ok(value)
        })
    }
    pub fn workspace_artifacts(&self) -> PResult<Value> {
        self.transaction(|tx|{
        let mut stmt=tx.prepare(&format!("SELECT {META} FROM workspace_artifacts a JOIN runs r ON r.id=a.run_id ORDER BY a.created_at DESC,a.id LIMIT 200"))?;
        let items=stmt.query_map([],|r|r.get::<_,String>(0))?.map(|r|Ok(serde_json::from_str::<Value>(&r?)?)).collect::<PResult<Vec<_>>>()?;Ok(json!({"artifacts":items}))
    })
    }
    pub fn workspace_artifact(&self, id: &str) -> PResult<Value> {
        self.transaction(|tx| artifact_in(tx, id, true))
    }
    pub fn run_workspace_artifacts(&self, run: &str) -> PResult<Value> {
        self.transaction(|tx|{
        let instance=scope(tx,run)?;
        let mut stmt=tx.prepare(&format!("SELECT {META} FROM workspace_artifacts a JOIN runs r ON r.id=a.run_id WHERE a.instance_id=? ORDER BY a.created_at DESC LIMIT 100"))?;
        let items=stmt.query_map(params![instance],|r|r.get::<_,String>(0))?.map(|r|Ok(serde_json::from_str::<Value>(&r?)?)).collect::<PResult<Vec<_>>>()?;Ok(json!({"artifacts":items,"untrusted_evidence":true}))
    })
    }
    pub fn read_workspace_artifact(&self, run: &str, id: &str) -> PResult<Value> {
        self.transaction(|tx| {
            let instance = scope(tx, run)?;
            let v = artifact_in(tx, id, true)?;
            if v["instance_id"] != instance {
                // Immutable snapshots are the stigmergy handoff: peers sharing
                // a board root (invitation or recruitment) may read them.
                let shared:bool=tx.query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM (SELECT root_id FROM board_agents WHERE instance_id=? UNION SELECT root_id FROM board_peer_roots WHERE instance_id=?) a
                       JOIN (SELECT root_id FROM board_agents WHERE instance_id=? UNION SELECT root_id FROM board_peer_roots WHERE instance_id=?) b ON a.root_id=b.root_id)",
                    params![&instance,&instance,v["instance_id"].as_str().unwrap(),v["instance_id"].as_str().unwrap()],
                    |r|r.get(0),
                )?;
                if !shared {
                    return Err(fault(403, "workspace_artifact_out_of_scope"));
                }
            }
            Ok(v)
        })
    }
}
fn artifact_in(tx: &Transaction<'_>, id: &str, body: bool) -> PResult<Value> {
    let (meta,content):(String,String)=tx.query_row(&format!("SELECT {META},a.content FROM workspace_artifacts a JOIN runs r ON r.id=a.run_id WHERE a.id=?"),[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.ok_or_else(||fault(404,"workspace_artifact_not_found"))?;
    let mut value: Value = serde_json::from_str(&meta)?;
    value["integrity_verified"] = json!(value["digest"] == digest(&content));
    value["untrusted_content"] = json!(true);
    if body {
        value["content"] = json!(content);
        if value["kind"] == "json" {
            value["value"] = serde_json::from_str(&content)?;
        }
    }
    Ok(value)
}
