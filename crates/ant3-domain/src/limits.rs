//! Local capacity defaults for a development workstation.
pub const INSTANCES: usize = 256;
pub const WORKERS: usize = 8;
pub const TASKS_PER_GOAL: usize = 64;
pub const DELEGATION_DEPTH: usize = 6;
pub const DELEGATION_CHILDREN: usize = 8;
pub const DELEGATION_DESCENDANTS: usize = 48;
/// Bounded automatic retries for a definitely-failed board task before it
/// settles as failed. Interrupted (outcome-unknown) Runs never auto-retry.
pub const BOARD_TASK_RETRIES: usize = 2;
