//! CCBR workspace actors, git worktree handling, planner, reconcile, validator.
//!
//! Mirrors `lib/workspace/` from Python v7.5.2.

pub mod actors;
pub mod binding;
pub mod git_worktree;
pub mod materializer;
pub mod models;
pub mod planner;
pub mod reconcile;
pub mod validator;

pub use actors::resolve_workspace_actor;

use thiserror::Error;

/// Errors raised by the workspace crate.
#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("workspace error: {0}")]
    Workspace(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] ccbr_storage::StorageError),
    #[error("project discovery error: {0}")]
    ProjectDiscovery(#[from] ccbr_project::discovery::ProjectDiscoveryError),
    #[error("agent error: {0}")]
    Agents(#[from] ccbr_agents::AgentError),
}

/// Crate-local result type.
pub type Result<T> = std::result::Result<T, WorkspaceError>;

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
