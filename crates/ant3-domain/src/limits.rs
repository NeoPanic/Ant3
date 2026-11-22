//! Local capacity defaults for a development workstation.
pub const INSTANCES: usize = 256;
pub const WORKERS: usize = 64;
/// Concurrently open root tasks (independent of tree size).
pub const BOARD_ACTIVE_ROOTS: usize = 64;
/// Maximum split depth below a root task.
pub const BOARD_SPLIT_DEPTH: usize = 3;
/// Maximum children published by a single split.
pub const BOARD_SPLIT_CHILDREN: usize = 64;
