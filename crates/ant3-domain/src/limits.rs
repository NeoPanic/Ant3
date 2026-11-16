//! Local capacity defaults for a development workstation.
pub const INSTANCES: usize = 256;
pub const WORKERS: usize = 8;
/// Maximum tasks in one shared-board root tree.
pub const BOARD_TASKS_PER_ROOT: usize = 64;
/// Maximum split depth below a root task.
pub const BOARD_SPLIT_DEPTH: usize = 6;
/// Maximum children published by a single split.
pub const BOARD_SPLIT_CHILDREN: usize = 8;
/// Bounded automatic retries for a definitely-failed board task before it
/// settles as failed. Interrupted (outcome-unknown) Runs never auto-retry.
pub const BOARD_TASK_RETRIES: usize = 2;