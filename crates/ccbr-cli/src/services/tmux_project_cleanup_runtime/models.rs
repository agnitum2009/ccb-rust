//! Mirrors Python `lib/cli/services/tmux_project_cleanup_runtime/models.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTmuxCleanupSummary {
    pub socket_name: Option<String>,
    pub owned_panes: Vec<String>,
    pub active_panes: Vec<String>,
    pub orphaned_panes: Vec<String>,
    pub killed_panes: Vec<String>,
}
