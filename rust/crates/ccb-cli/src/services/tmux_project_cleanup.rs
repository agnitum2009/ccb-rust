//! Mirrors Python `lib/cli/services/tmux_project_cleanup.py`.

pub use crate::services::tmux_project_cleanup_runtime::{
    cleanup_project_tmux_orphans, cleanup_project_tmux_orphans_by_socket, kill_project_tmux_panes,
    list_project_tmux_panes_owned as list_project_tmux_panes, ProjectTmuxCleanupSummary,
};

/// Alias for the runtime module so the public API matches the file layout.
pub mod tmux_project_cleanup_runtime {
    pub use crate::services::tmux_project_cleanup_runtime::*;
}
