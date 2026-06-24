//! CCBR pane registry for tmux pane identity and lifecycle.
//!
//! Mirrors `lib/pane_registry_runtime/` from Python v7.5.2.

pub mod api;
pub mod common;
pub mod common_runtime;
pub mod lookup;
pub mod lookup_project;
pub mod lookup_records;
pub mod writes;

pub use api::{
    load_registry_by_claude_pane, load_registry_by_project_id, load_registry_by_session_id,
    upsert_registry,
};
pub use common::{
    get_providers_map, registry_path_for_session, REGISTRY_PREFIX, REGISTRY_SUFFIX,
    REGISTRY_TTL_SECONDS,
};

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_smoke() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
