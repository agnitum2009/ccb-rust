//! Mirrors Python `lib/cli/services/daemon_runtime/models.py`.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::services::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;

/// Mirrors Python `LeaseHealth` enum from `ccbd.models_runtime.mount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseHealth {
    Healthy,
    Degraded,
    Stale,
    Unmounted,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProjectDaemonInspection {
    Direct(serde_json::Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CcbdServiceError(pub String);

impl fmt::Display for CcbdServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CcbdServiceError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonHandle {
    pub client: Option<serde_json::Value>,
    pub inspection: serde_json::Value,
    #[serde(default)]
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalPingSummary {
    pub project_id: String,
    pub mount_state: String,
    pub desired_state: String,
    pub health: String,
    pub generation: Option<usize>,
    pub project_anchor_path: Option<String>,
    pub runtime_state_root: Option<String>,
    pub runtime_root_kind: Option<String>,
    pub runtime_relocation_reason: Option<String>,
    pub runtime_filesystem_hint: Option<String>,
    pub runtime_marker_status: Option<String>,
    pub socket_path: Option<String>,
    pub preferred_socket_path: Option<String>,
    pub effective_socket_path: Option<String>,
    pub socket_root_kind: Option<String>,
    pub socket_fallback_reason: Option<String>,
    pub socket_filesystem_hint: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_preferred_socket_path: Option<String>,
    pub tmux_effective_socket_path: Option<String>,
    pub tmux_socket_root_kind: Option<String>,
    pub tmux_socket_fallback_reason: Option<String>,
    pub tmux_socket_filesystem_hint: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub pid_alive: bool,
    pub socket_connectable: bool,
    pub heartbeat_fresh: bool,
    pub takeover_allowed: bool,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_progress_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_deadline_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shutdown_intent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillSummary {
    pub project_id: String,
    pub state: String,
    pub socket_path: String,
    pub forced: bool,
    #[serde(default)]
    pub cleanup_summaries: Vec<ProjectTmuxCleanupSummary>,
    #[serde(default)]
    pub worktree_warnings: Vec<serde_json::Value>,
}
