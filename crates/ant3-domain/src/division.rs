//! Response-threshold division of labor and crowding avoidance.
//!
//! Ant-inspired local rules: a preset's response threshold adapts with
//! experience (success lowers it, definite failure raises it), and a task's
//! stimulus is its trail strength diluted by how many workers already crowd
//! its root. Low-threshold, experienced presets respond to weaker stimuli;
//! crowded branches lose attractiveness so workers spread across the board.
use crate::traces::quantize_strength;

/// Base threshold for an inexperienced preset.
pub const THRESHOLD_BASE: f64 = 0.5;
pub const THRESHOLD_MIN: f64 = 0.05;
pub const THRESHOLD_MAX: f64 = 0.95;

/// Adaptive response threshold: each success multiplies by 1/1.25, each
/// definite failure by 1.25, clamped to a sane band. Outcome-unknown Runs
/// are not experience and must not be recorded.
pub fn response_threshold(successes: u32, failures: u32) -> f64 {
    let delta = failures as i32 - successes as i32;
    quantize_strength((THRESHOLD_BASE * 1.25_f64.powi(delta)).clamp(THRESHOLD_MIN, THRESHOLD_MAX))
}

/// Task stimulus: trail strength diluted by crowding. `crowd` is the number
/// of queued/running workers already engaged on the task's root.
pub fn stimulus(strength: f64, crowd: usize) -> f64 {
    quantize_strength(strength.max(0.0) / (1 + crowd) as f64)
}

/// Classic threshold response probability s²/(s²+θ²): monotone in stimulus,
/// anti-monotone in threshold, 0.5 at s=θ. Used as evidence, not authority.
pub fn response_probability(stimulus: f64, threshold: f64) -> f64 {
    let s = stimulus.max(0.0);
    let t = threshold.max(1e-9);
    quantize_strength(s * s / (s * s + t * t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_adapt_with_experience_and_stay_bounded() {
        assert_eq!(response_threshold(0, 0), THRESHOLD_BASE);
        assert!(response_threshold(3, 0) < response_threshold(1, 0));
        assert!(response_threshold(0, 3) > response_threshold(0, 1));
        assert_eq!(response_threshold(2, 2), THRESHOLD_BASE);
        assert_eq!(response_threshold(100, 0), THRESHOLD_MIN);
        assert_eq!(response_threshold(0, 100), THRESHOLD_MAX);
    }

    #[test]
    fn crowding_dilutes_stimulus() {
        assert_eq!(stimulus(8.0, 0), 8.0);
        assert_eq!(stimulus(8.0, 1), 4.0);
        assert_eq!(stimulus(8.0, 3), 2.0);
        assert_eq!(stimulus(-1.0, 0), 0.0);
        assert_eq!(stimulus(0.0, 5), 0.0);
    }

    #[test]
    fn response_probability_is_monotone_and_half_at_threshold() {
        assert_eq!(response_probability(0.5, 0.5), 0.5);
        assert!(response_probability(2.0, 0.5) > response_probability(1.0, 0.5));
        assert!(response_probability(1.0, 0.2) > response_probability(1.0, 0.8));
        assert_eq!(response_probability(0.0, 0.5), 0.0);
    }

    #[test]
    fn experienced_presets_respond_to_weaker_stimuli() {
        // A specialist (many successes) responds to a faint trail that a
        // novice ignores: division of labor emerges from local experience.
        let specialist = response_threshold(8, 0);
        let novice = response_threshold(0, 0);
        let faint = stimulus(0.4, 0);
        assert!(response_probability(faint, specialist) > response_probability(faint, novice));
    }
}