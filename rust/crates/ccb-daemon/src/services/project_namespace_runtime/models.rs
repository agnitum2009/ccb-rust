//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/models.py`.
//! 1:1 file alignment stub.

use serde::{Deserialize, Serialize};

/// Mirrors Python `lib/ccbd/services/project_namespace_state_runtime/models.py`.
/// Placeholder: minimal fields used by `records.py`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNamespaceState {
    pub project_id: String,
    pub namespace_epoch: i64,
    pub tmux_socket_path: String,
    pub tmux_session_name: String,
    #[serde(default = "default_layout_version")]
    pub layout_version: i64,
    #[serde(default)]
    pub layout_signature: Option<String>,
    #[serde(default)]
    pub control_window_name: Option<String>,
    #[serde(default)]
    pub control_window_id: Option<String>,
    #[serde(default)]
    pub workspace_window_name: Option<String>,
    #[serde(default)]
    pub workspace_window_id: Option<String>,
    #[serde(default = "default_workspace_epoch")]
    pub workspace_epoch: i64,
    #[serde(default = "default_ui_attachable")]
    pub ui_attachable: bool,
    #[serde(default)]
    pub last_started_at: Option<String>,
    #[serde(default)]
    pub last_destroyed_at: Option<String>,
    #[serde(default)]
    pub last_destroy_reason: Option<String>,
}

fn default_layout_version() -> i64 {
    1
}

fn default_workspace_epoch() -> i64 {
    1
}

fn default_ui_attachable() -> bool {
    true
}

impl ProjectNamespaceState {
    pub fn with_destroyed(
        &self,
        occurred_at: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let reason = reason.into();
        let reason = if reason.trim().is_empty() {
            "destroyed".to_string()
        } else {
            reason
        };
        Self {
            project_id: self.project_id.clone(),
            namespace_epoch: self.namespace_epoch,
            tmux_socket_path: self.tmux_socket_path.clone(),
            tmux_session_name: self.tmux_session_name.clone(),
            layout_version: self.layout_version,
            layout_signature: self.layout_signature.clone(),
            control_window_name: self.control_window_name.clone(),
            control_window_id: self.control_window_id.clone(),
            workspace_window_name: self.workspace_window_name.clone(),
            workspace_window_id: self.workspace_window_id.clone(),
            workspace_epoch: self.workspace_epoch,
            ui_attachable: false,
            last_started_at: self.last_started_at.clone(),
            last_destroyed_at: Some(occurred_at.into()),
            last_destroy_reason: Some(reason),
        }
    }
}

/// Mirrors Python `lib/ccbd/services/project_namespace_state_runtime/models.py`.
/// Placeholder: minimal fields used by `records.py`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNamespaceEvent {
    pub event_kind: String,
    pub project_id: String,
    pub occurred_at: String,
    #[serde(default)]
    pub namespace_epoch: Option<i64>,
    #[serde(default)]
    pub tmux_socket_path: Option<String>,
    #[serde(default)]
    pub tmux_session_name: Option<String>,
    #[serde(default)]
    pub details: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNamespace {
    pub project_id: String,
    pub namespace_epoch: i64,
    pub tmux_socket_path: String,
    pub tmux_session_name: String,
    pub layout_version: i64,
    pub layout_signature: Option<String>,
    pub control_window_name: Option<String>,
    pub control_window_id: Option<String>,
    pub workspace_window_name: Option<String>,
    pub workspace_window_id: Option<String>,
    pub workspace_epoch: i64,
    pub ui_attachable: bool,
    #[serde(default)]
    pub created_this_call: bool,
    #[serde(default)]
    pub workspace_recreated_this_call: bool,
}

impl ProjectNamespace {
    pub fn from_state(state: &ProjectNamespaceState) -> Self {
        Self {
            project_id: state.project_id.clone(),
            namespace_epoch: state.namespace_epoch,
            tmux_socket_path: state.tmux_socket_path.clone(),
            tmux_session_name: state.tmux_session_name.clone(),
            layout_version: state.layout_version,
            layout_signature: state.layout_signature.clone(),
            control_window_name: state.control_window_name.clone(),
            control_window_id: state.control_window_id.clone(),
            workspace_window_name: state.workspace_window_name.clone(),
            workspace_window_id: state.workspace_window_id.clone(),
            workspace_epoch: state.workspace_epoch,
            ui_attachable: state.ui_attachable,
            created_this_call: false,
            workspace_recreated_this_call: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectNamespaceDestroySummary {
    pub project_id: String,
    pub namespace_epoch: Option<i64>,
    pub tmux_socket_path: String,
    pub tmux_session_name: String,
    pub destroyed: bool,
    pub reason: String,
}
