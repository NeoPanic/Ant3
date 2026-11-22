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

/// Oscillation suppression: rework chains cool down exponentially and stop
/// at a bounded chain length, handing the task back to human review instead
/// of looping between worker and reviewer forever.
pub const REWORK_CHAIN_LIMIT: usize = 4;
pub const REWORK_COOLDOWN_BASE_MS: i64 = 60_000;
pub const REWORK_COOLDOWN_MAX_MS: i64 = 900_000;

/// Cooldown before a reworked task is offered again: base * 2^(chain-1),
/// capped. Chain counts consecutive changes_requested rounds.
pub fn rework_cooldown_ms(chain: usize) -> i64 {
    if chain == 0 {
        return 0;
    }
    (REWORK_COOLDOWN_BASE_MS * 2_i64.pow((chain as u32).saturating_sub(1).min(8)))
        .min(REWORK_COOLDOWN_MAX_MS)
}

/// Cost-aware threshold: a preset's energy efficiency scales its threshold.
/// efficiency = mean_energy_per_success / ENERGY_RESERVE (clamped 0.25..4).
/// Cheap specialists (efficiency < 1) get lower thresholds and respond to
/// fainter stimuli; expensive presets need stronger evidence of payoff.
pub fn cost_adjusted_threshold(threshold: f64, efficiency: f64) -> f64 {
    let factor = efficiency.clamp(0.25, 4.0);
    quantize_strength((threshold * factor).clamp(THRESHOLD_MIN, THRESHOLD_MAX))
}

/// Self-selection mode: idle peers scan the open board and claim work
/// themselves; the control plane keeps only physics (capacity, energy,
/// atomicity). The wandering pool reproduces toward successful presets and
/// starves out wasteful ones — colony composition emerges from experience.
pub const SELF_SELECT_POOL_MAX: usize = 64;
/// Idle energy burn per standing instance per minute (wander cost).
pub const IDLE_BURN_PER_MIN: i64 = 0;
/// A preset with this many consecutive failures and no recent success is
/// retired from the wandering pool.
pub const POOR_PRESET_FAILURES: u32 = 3;

/// Whether an idle peer responds to a task: probability s²/(s²+θ²) drawn
/// against a uniform sample in [0,1). Deterministic given the sample.
pub fn responds(stimulus: f64, threshold: f64, sample: f64) -> bool {
    sample < response_probability(stimulus, threshold)
}

/// Pool reproduction target for a preset: share of the pool proportional to
/// verified successes (Laplace-smoothed so newcomers get a chance), minus
/// presets retired for sustained failure. At least one slot stays open for
/// exploration when any preset is eligible.
pub fn pool_targets(experience: &[(String, u32, u32)], pool_max: usize) -> Vec<(String, usize)> {
    let eligible: Vec<_> = experience
        .iter()
        .filter(|(_, s, f)| !(*f >= POOR_PRESET_FAILURES && *s == 0))
        .collect();
    if eligible.is_empty() || pool_max == 0 {
        return Vec::new();
    }
    let total: f64 = eligible.iter().map(|(_, s, _)| (*s as f64) + 1.0).sum();
    let mut out: Vec<(String, usize)> = eligible
        .iter()
        .map(|(name, s, _)| {
            let share = ((*s as f64) + 1.0) / total * pool_max as f64;
            (name.clone(), share.floor() as usize)
        })
        .collect();
    // Distribute remaining slots by largest fractional remainder.
    let used: usize = out.iter().map(|(_, n)| n).sum();
    let mut remainders: Vec<(usize, f64)> = eligible
        .iter()
        .enumerate()
        .map(|(i, (_, s, _))| {
            let share = ((*s as f64) + 1.0) / total * pool_max as f64;
            (i, share - share.floor())
        })
        .collect();
    remainders.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (i, _) in remainders.into_iter().take(pool_max.saturating_sub(used)) {
        out[i].1 += 1;
    }
    out
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
    fn response_draws_are_monotone_in_stimulus() {
        assert!(responds(4.0, 0.5, 0.5));
        assert!(!responds(0.1, 0.9, 0.5));
        assert!(responds(1.0, 0.5, 0.79));
        assert!(!responds(1.0, 0.5, 0.81));
    }

    #[test]
    fn pool_reproduces_toward_success_and_retires_sustained_failure() {
        let exp = vec![
            ("good".to_string(), 8u32, 0u32),
            ("new".to_string(), 0u32, 0u32),
            ("bad".to_string(), 0u32, 5u32),
        ];
        let targets = pool_targets(&exp, 8);
        let good = targets.iter().find(|(n, _)| n == "good").unwrap().1;
        let new = targets.iter().find(|(n, _)| n == "new").unwrap().1;
        assert!(good > new);
        assert_eq!(good + new, 8);
        assert!(!targets.iter().any(|(n, _)| n == "bad"));
        // All-failing population: nobody reproduces.
        assert!(pool_targets(&[("bad".to_string(), 0, 4)], 8).is_empty());
        // Smoothed newcomers still get slots in a fresh colony.
        let fresh = pool_targets(&[("a".to_string(), 0, 0), ("b".to_string(), 0, 0)], 4);
        assert_eq!(fresh.iter().map(|(_, n)| n).sum::<usize>(), 4);
    }

    #[test]
    fn rework_cooldown_grows_and_caps() {
        assert_eq!(rework_cooldown_ms(0), 0);
        assert_eq!(rework_cooldown_ms(1), REWORK_COOLDOWN_BASE_MS);
        assert_eq!(rework_cooldown_ms(2), REWORK_COOLDOWN_BASE_MS * 2);
        assert_eq!(rework_cooldown_ms(20), REWORK_COOLDOWN_MAX_MS);
    }

    #[test]
    fn cheap_presets_get_lower_thresholds_than_expensive_ones() {
        let base = response_threshold(2, 0);
        let cheap = cost_adjusted_threshold(base, 0.5);
        let expensive = cost_adjusted_threshold(base, 2.0);
        assert!(cheap < base && base < expensive);
        assert_eq!(cost_adjusted_threshold(base, 1.0), base);
        // Clamped: absurd efficiencies cannot escape the band.
        assert!(cost_adjusted_threshold(base, 0.0) >= THRESHOLD_MIN);
        assert!(cost_adjusted_threshold(base, 100.0) <= THRESHOLD_MAX);
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