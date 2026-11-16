//! Root-scoped shared notes: the swarm's versioned blackboard.
//!
//! Peers publish findings under stable keys with compare-and-swap revisions.
//! Contradicting claims on the same key are not silently overwritten: the
//! platform adjudicates by evidence support (trace strength cited by each
//! side), keeping the better-supported claim active and marking the other
//! superseded. Notes are untrusted evidence, never instructions.
use crate::*;

pub const NOTES_PER_ROOT: usize = 64;
pub const NOTE_WRITES_PER_RUN: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteInput {
    pub key: String,
    pub body: String,
    /// Revision the writer believes is current; 0 creates a new note.
    pub expected_revision: i64,
    /// Trace keys whose strength supports this claim (evidence backing).
    #[serde(default)]
    pub support: Vec<String>,
}
impl NoteInput {
    pub fn validate(&self) -> PResult<()> {
        if !valid_name(&self.key)
            || self.body.trim().is_empty()
            || self.body.len() > 2000
            || !(0..128).contains(&self.expected_revision)
            || self.support.len() > 8
            || self.support.iter().any(|k| !valid_name(k))
            || self.support.iter().collect::<std::collections::HashSet<_>>().len()
                != self.support.len()
        {
            return Err(fault(400, "invalid_board_note"));
        }
        Ok(())
    }
}

/// Adjudication outcome for a conflicting write (expected_revision stale).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adjudication {
    /// New claim has strictly stronger evidence: it supersedes the current.
    Supersede,
    /// Current claim keeps the key; the new claim is recorded as superseded.
    Keep,
    /// Equal support: first-writer-wins, newcomer recorded as superseded.
    TieKeep,
}

pub fn adjudicate(current_support: f64, challenger_support: f64) -> Adjudication {
    // Scores evaporate between reads; equal-strength deposits made moments
    // apart differ by micro-decay. A relative tolerance keeps such claims
    // tied so arrival jitter cannot flip an adjudication.
    let eps = 1e-4 * current_support.abs().max(challenger_support.abs()).max(1.0);
    if challenger_support - current_support > eps {
        Adjudication::Supersede
    } else if (challenger_support - current_support).abs() <= eps {
        Adjudication::TieKeep
    } else {
        Adjudication::Keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stronger_evidence_supersedes_and_ties_keep_the_incumbent() {
        assert_eq!(adjudicate(4.0, 8.0), Adjudication::Supersede);
        assert_eq!(adjudicate(8.0, 4.0), Adjudication::Keep);
        assert_eq!(adjudicate(4.0, 4.0), Adjudication::TieKeep);
        assert_eq!(adjudicate(0.0, 0.0), Adjudication::TieKeep);
        assert_eq!(adjudicate(2.0, 1.999999), Adjudication::TieKeep);
        assert_eq!(adjudicate(1.999998, 1.999999), Adjudication::TieKeep);
        assert_eq!(adjudicate(2.0, 2.1), Adjudication::Supersede);
    }

    #[test]
    fn note_inputs_are_bounded() {
        let mut n = NoteInput { key: "finding.x".into(), body: "claim".into(), expected_revision: 0, support: vec!["trace.a".into()] };
        assert!(n.validate().is_ok());
        n.support = vec!["a".into(), "a".into()];
        assert!(n.validate().is_err());
        n.support = vec![];
        n.body = " ".into();
        assert!(n.validate().is_err());
    }
}
