//! Budget-as-energy: the swarm's selection pressure.
//!
//! Every root task carries a finite energy pool. Dispatching work reserves
//! energy atomically; settlement bills actual model usage (unknown usage is
//! billed conservatively at the reservation). Verified success refunds
//! energy — efficient paths earn their budget back, wasteful paths starve
//! and are culled from offering. This turns accounting into an emergence
//! mechanism: strategies that convert energy into accepted results survive.
use crate::*;

/// Default root energy when the publisher does not specify one.
pub const ENERGY_DEFAULT: i64 = 100_000;
pub const ENERGY_MAX: i64 = 10_000_000;
/// Reservation per dispatched Run (worker or auditor).
pub const ENERGY_RESERVE: i64 = 2_000;
/// Conservative bill when the provider reports no usage.
pub const ENERGY_UNKNOWN_COST: i64 = 1_500;
/// Refund on independent-audit acceptance: verified success earns energy.
pub const ENERGY_ACCEPT_REFUND: i64 = 4_000;
/// Refund on plain completion of a child contribution.
pub const ENERGY_COMPLETE_REFUND: i64 = 500;
/// Tokens per energy unit (input and output weighted equally; cached input
/// is free — providers bill it separately and it signals efficient reuse).
pub const TOKENS_PER_ENERGY: i64 = 10;

/// Energy cost of a model usage report. Unknown usage bills conservatively.
pub fn usage_cost(input: u64, output: u64, cached_input: Option<u64>) -> i64 {
    let billable = input.saturating_sub(cached_input.unwrap_or(0)) + output;
    let cost = (billable as i64 + TOKENS_PER_ENERGY - 1) / TOKENS_PER_ENERGY;
    cost.max(1)
}

/// Validate a publisher-supplied energy budget.
pub fn validate_budget(budget: i64) -> PResult<()> {
    if !(ENERGY_RESERVE..=ENERGY_MAX).contains(&budget) {
        return Err(fault(400, "invalid_energy_budget"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_cost_rounds_up_and_ignores_cached_input() {
        assert_eq!(usage_cost(95, 5, None), 10);
        assert_eq!(usage_cost(96, 5, None), 11);
        assert_eq!(usage_cost(1000, 200, Some(800)), 40);
        assert_eq!(usage_cost(0, 0, None), 1);
        assert_eq!(usage_cost(10, 0, Some(50)), 1);
    }

    #[test]
    fn budgets_are_bounded() {
        assert!(validate_budget(ENERGY_DEFAULT).is_ok());
        assert!(validate_budget(ENERGY_RESERVE - 1).is_err());
        assert!(validate_budget(ENERGY_MAX + 1).is_err());
    }
}