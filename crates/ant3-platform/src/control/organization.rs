//! Built-in organization capabilities share the same immutable binding contract as packages.
use crate::{domain::*, store::Store};
use ant3::Tool;
use serde_json::{Value,json};
use std::sync::Arc;
pub fn codex_spec() -> ant3::ToolSpec {
        ant3::ToolSpec {
            name: "external.codex".into(),
            description: "Delegate a concrete task to the local Codex CLI in your instance workspace. Supply the goal and acceptance criteria. Review its returned output before continuing.".into(),
            parameters: json!({"type":"object","additionalProperties":false,"required":["task"],"properties":{"task":{"type":"string","minLength":1,"maxLength":32000}}}),
            mutates: true,
        }
}
pub fn codex_package() -> Value {
    use sha2::{Digest, Sha256};
    let mut package = json!({"id":"external.codex","version":"1.0.0","kind":"builtin","description":"委托本机 Codex CLI 在实例工作区执行任务","instructions":codex_spec().description,"tools":[codex_spec()]});
    package["digest"] = json!(format!("sha256:{}", Sha256::digest(package.to_string().as_bytes()).iter().map(|b| format!("{b:02x}")).collect::<String>()));
    package
}
pub fn packages() -> Vec<Value> {
    vec![codex_package()]
}
pub fn system_packages() -> Vec<Value> {
    vec![crate::messaging::package(), crate::workspaces::package()]
}
pub fn package(id: &str) -> Option<Value> {
    match id {
        "external.codex" => Some(codex_package()),
        "platform.workspace" => Some(crate::workspaces::package()),
        "platform.messaging" => Some(crate::messaging::package()),
        _ => None,
    }
}
pub fn tools(snapshot: &Value, store: Store, run: &str) -> PResult<Vec<Arc<dyn Tool>>> {
    let id = snapshot["id"].as_str().unwrap_or_default();
    if package(id).as_ref() != Some(snapshot) {
        return Err(fault(409, "organization_plugin_snapshot_unsupported"));
    }
    Ok(match id {
        "platform.workspace" => crate::workspaces::tools(store, run),
        "platform.messaging" => vec![crate::messaging::tool(store, run)],
        _ => return Err(fault(404, "builtin_plugin_not_found")),
    })
}