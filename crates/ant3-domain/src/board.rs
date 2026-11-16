use crate::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishInput {
    pub title: String,
    pub instructions: String,
    pub acceptance: String,
    /// Optional independent verification for a root task. When present, a
    /// fresh reviewer instance audits the frozen result against the acceptance
    /// criteria before the result can be auto-accepted. Humans keep final say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<BoardVerification>,
}
impl PublishInput {
    pub fn validate(&self) -> PResult<()> {
        if self.title.trim().is_empty()
            || self.title.len() > 200
            || self.instructions.trim().is_empty()
            || self.instructions.len() > 8000
            || self.acceptance.trim().is_empty()
            || self.acceptance.len() > 8000
        {
            return Err(fault(400, "invalid_board_task"));
        }
        if let Some(v) = &self.verification {
            v.validate()?;
        }
        Ok(())
    }
}

/// Operator-chosen reviewer for a root task. The acceptance text becomes the
/// single required check; the reviewer is always a fresh, independent
/// instance — never the worker that produced the result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardVerification {
    pub preset_name: String,
    pub preset_version: String,
    #[serde(default)]
    pub max_auto_reworks: usize,
}
impl BoardVerification {
    pub fn validate(&self) -> PResult<()> {
        if !valid_name(&self.preset_name)
            || !valid_name(&self.preset_version)
            || self.max_auto_reworks > 2
        {
            return Err(fault(400, "invalid_board_verification"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitInput {
    pub tasks: Vec<PublishInput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewInput {
    pub run_id: String,
    pub decision: ReviewDecision,
    pub feedback: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Accepted,
    ChangesRequested,
}
impl ReviewInput {
    pub fn validate(&self) -> PResult<()> {
        if self.run_id.is_empty()
            || self.run_id.len() > 80
            || self.feedback.len() > 8000
            || (self.decision == ReviewDecision::ChangesRequested
                && self.feedback.trim().is_empty())
        {
            return Err(fault(400, "invalid_board_review"));
        }
        Ok(())
    }
}
