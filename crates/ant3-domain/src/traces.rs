//! Pheromone-style trace dynamics for the shared task board.
//!
//! Traces are the stigmergy medium: verified events and bounded agent signals
//! reinforce them, and strength evaporates with a fixed half-life. Decay is
//! computed lazily at read/deposit time, so no background job is required.
use crate::*;

/// Strength halves every 30 minutes without reinforcement.
pub const TRACE_HALF_LIFE_MS: i64 = 30 * 60 * 1000;
/// Saturation cap: positive feedback cannot grow without bound.
pub const TRACE_MAX_STRENGTH: f64 = 100.0;
pub const TRACES_PER_ROOT: usize = 64;
pub const TRACE_DEPOSITS_PER_RUN: usize = 8;
/// Fixed deposit weights. Agents cannot choose amounts; only the platform
/// decides how much a verified event is worth.
pub const TRACE_AGENT_AMOUNT: f64 = 2.0;
pub const TRACE_REUSE_AMOUNT: f64 = 1.0;
pub const TRACE_COMPLETION_AMOUNT: f64 = 4.0;
pub const TRACE_ACCEPTED_AMOUNT: f64 = 12.0;

/// Kinds an Agent may deposit. Quality-like kinds (`reuse`, `completion`,
/// `accepted`) are platform-only and tied to tool receipts or human review.
pub const AGENT_TRACE_KINDS: [&str; 3] = ["finding", "path", "blocker"];
pub const RESULT_TRACE_KIND: &str = "result";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceInput {
    pub key: String,
    pub kind: String,
    #[serde(default)]
    pub note: String,
}
impl TraceInput {
    pub fn validate(&self) -> PResult<()> {
        if !valid_name(&self.key)
            || !AGENT_TRACE_KINDS.contains(&self.kind.as_str())
            || self.note.len() > 500
        {
            return Err(fault(400, "invalid_board_trace"));
        }
        Ok(())
    }
}

/// Evaporated strength at `now`. Never negative; clock skew cannot amplify.
/// Quantized to six decimals: pheromone signals are low-bandwidth, and short
/// decimal values round-trip exactly through JSON session storage.
pub fn effective_strength(strength: f64, updated_at: i64, now: i64) -> f64 {
    let age = now.saturating_sub(updated_at).max(0) as f64;
    quantize((strength * 0.5_f64.powf(age / TRACE_HALF_LIFE_MS as f64)).max(0.0))
}

/// Reinforce with saturation. Callers must evaporate the old strength first.
pub fn reinforce(strength: f64, amount: f64) -> f64 {
    quantize((strength + amount).clamp(0.0, TRACE_MAX_STRENGTH))
}

fn quantize(strength: f64) -> f64 {
    (strength * 1e6).round() / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_halves_every_half_life_and_never_goes_negative() {
        let base = 40.0;
        let t0 = 1_000_000;
        assert_eq!(effective_strength(base, t0, t0), base);
        let one = effective_strength(base, t0, t0 + TRACE_HALF_LIFE_MS);
        assert!((one - base / 2.0).abs() < 1e-9);
        let two = effective_strength(base, t0, t0 + 2 * TRACE_HALF_LIFE_MS);
        assert!((two - base / 4.0).abs() < 1e-9);
        assert_eq!(effective_strength(base, t0, t0 - 5_000), base);
        assert_eq!(effective_strength(0.0, t0, t0 + TRACE_HALF_LIFE_MS), 0.0);
    }

    #[test]
    fn reinforcement_saturates_and_stays_non_negative() {
        assert_eq!(reinforce(99.0, 12.0), TRACE_MAX_STRENGTH);
        assert_eq!(reinforce(0.0, 2.0), 2.0);
        assert_eq!(reinforce(-5.0, 1.0), 0.0);
    }

    #[test]
    fn quantized_strength_round_trips_through_json() {
        // Session history is append-only JSON; a strength that drifts by one
        // ULP on reload would break validate_append for every board Run.
        let t0 = 1_700_000_000_000;
        for age in [1, 1234, 999_999, TRACE_HALF_LIFE_MS, 7 * TRACE_HALF_LIFE_MS] {
            let strength = effective_strength(12.0, t0, t0 + age);
            let encoded = serde_json::to_string(&strength).unwrap();
            let decoded: f64 = serde_json::from_str(&encoded).unwrap();
            assert_eq!(strength, decoded, "age={age} encoded={encoded}");
        }
    }

    #[test]
    fn decay_then_reinforce_is_order_sensitive() {
        let t0 = 0;
        let later = TRACE_HALF_LIFE_MS;
        // 40 decays to 20, then +12 = 32; naive add-then-decay would give 26.
        let decayed = effective_strength(40.0, t0, later);
        assert!((reinforce(decayed, 12.0) - 32.0).abs() < 1e-9);
    }

    #[test]
    fn trace_input_accepts_only_bounded_agent_kinds() {
        let mut input = TraceInput {
            key: "source.arxiv".into(),
            kind: "finding".into(),
            note: "primary evidence".into(),
        };
        assert!(input.validate().is_ok());
        for kind in ["reuse", "completion", "accepted", "result", ""] {
            input.kind = kind.into();
            assert!(input.validate().is_err());
        }
        input.kind = "path".into();
        input.note = "x".repeat(501);
        assert!(input.validate().is_err());
        input.note = String::new();
        input.key = "bad key!".into();
        assert!(input.validate().is_err());
    }
}