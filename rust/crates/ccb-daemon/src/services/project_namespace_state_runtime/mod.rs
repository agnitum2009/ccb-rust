//! Mirrors Python `lib/ccbd/services/project_namespace_state_runtime/`.

pub mod common;
pub mod models;
pub mod stores;

pub use models::{ProjectNamespaceEvent, ProjectNamespaceState};
pub use stores::{next_namespace_epoch, ProjectNamespaceEventStore, ProjectNamespaceStateStore};
