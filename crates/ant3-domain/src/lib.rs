use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub type PResult<T> = Result<T, Fault>;

#[derive(Debug)]
pub struct Fault(pub u16, pub String);
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}
impl std::error::Error for Fault {}
impl From<serde_json::Error> for Fault {
    fn from(_: serde_json::Error) -> Self {
        Self(500, "stored_data_invalid".into())
    }
}
pub fn fault(code: u16, message: &str) -> Fault {
    Fault(code, message.into())
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub fn id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    // Legacy persisted snapshots and offline test fixtures; unavailable in the service.
    Demo,
    OpenaiCompatible,
    OpenaiResponses,
    Anthropic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginRef {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub name: String,
    pub version: String,
    pub purpose: String,
    pub objective: String,
    pub instructions: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub plugin: Option<PluginRef>,
}
impl Preset {
    pub fn validate(&self) -> PResult<()> {
        if !valid_name(&self.name)
            || !valid_name(&self.version)
            || self.purpose.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.instructions.trim().is_empty()
            || self.purpose.len() > 2000
            || self.objective.len() > 8000
            || self.instructions.len() > 32000
        {
            return Err(fault(400, "invalid_preset"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub id: String,
    pub version: String,
    pub description: String,
    pub instructions: String,
}
impl Bundle {
    pub fn validate(&self) -> PResult<()> {
        if !valid_name(&self.id)
            || !valid_name(&self.version)
            || self.description.trim().is_empty()
            || self.description.len() > 2000
            || self.instructions.trim().is_empty()
            || self.instructions.len() > 32000
        {
            return Err(fault(400, "invalid_prompt_bundle"));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    pub preset_name: String,
    pub preset_version: String,
    pub count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunInput {
    pub text: String,
}

pub fn outcome_unknown(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            // Only string codes count: an "error" object wrapper is a
            // container, not an outcome-unknown marker. Recursing into it
            // under the same key would flag every ordinary error report.
            (((key == "code" || key == "error")
                && value
                    .as_str()
                    .is_some_and(|s| s.contains("outcome_unknown")))
                || (key == "outcome_unknown" && value == &Value::Bool(true)))
                || (key != "arguments" && key != "call" && outcome_unknown(value))
        }),
        Value::Array(items) => items.iter().any(outcome_unknown),
        _ => false,
    }
}

pub mod board;
pub mod division;
pub mod fleets;
pub mod limits;
pub mod messaging;
pub mod traces;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn outcome_unknown_detects_only_real_unknown_markers() {
        assert!(outcome_unknown(&json!({"error":{"code":"interrupted_outcome_unknown"}})));
        assert!(outcome_unknown(&json!({"error":"external_outcome_unknown"})));
        assert!(outcome_unknown(&json!({"outcome_unknown":true})));
        assert!(outcome_unknown(&json!({"observations":[{"output":{"error":"tool_outcome_unknown"}}]})));
        // Ordinary error reports are definite failures, not unknown outcomes.
        assert!(!outcome_unknown(&json!({"error":{"code":"board_run_failed"}})));
        assert!(!outcome_unknown(&json!({"error":{"code":"timeout","message":"deadline"}})));
        assert!(!outcome_unknown(&json!({"output":"done"})));
        assert!(!outcome_unknown(&json!({"outcome_unknown":false})));
        // Tool arguments are untrusted data and never classify the outcome.
        assert!(!outcome_unknown(&json!({"arguments":{"error":"outcome_unknown"}})));
    }
}
