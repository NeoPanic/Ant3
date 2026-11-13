//! Local control-plane preview around the independent Ant³ runtime.
pub mod collaboration;
pub mod control;
pub mod domain;
pub use ant3_domain::limits;
pub use ant3_models as models;
pub use ant3_plugins as plugins;
pub mod scheduling;
pub mod storage;
pub mod workspaces;

// Preserve existing module paths while implementations move behind their boundaries.
pub use collaboration::{
    autonomy, channels, claims, coordination, delegation, messaging, research, verification,
};
pub use control::{fleets, lifecycle, organization, presets};
pub use scheduling::{board, decision, routing, traces};
pub use storage::sqlite as store;
pub use workspaces::artifacts;
