//! Mirrors Python `lib/ccbd/start_runtime/agent_runtime_models.py`.

#![allow(clippy::too_many_arguments)]

use serde::{Deserialize, Serialize};

/// A resolved provider binding for an agent runtime.
#[derive(Debug, Clone, Default)]
pub struct RuntimeBinding {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
    pub tmux_window_id: Option<String>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<String>,
    pub ccb_session_id: Option<String>,
}

/// Result returned by `ensure_agent_runtime` style helpers.
#[derive(Debug, Clone)]
pub struct EnsureAgentRuntimeResult {
    pub launched: bool,
    pub binding: Option<RuntimeBinding>,
}

/// Static call context for runtime resolution.
#[derive(Debug, Clone)]
pub struct Context {
    pub workspace_path: String,
}

/// Start command flags relevant to runtime launch.
#[derive(Debug, Clone)]
pub struct Command {
    pub restore: bool,
}

/// Agent specification.
#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub name: String,
    pub runtime_mode: String,
    pub provider: String,
}

/// Workspace plan.
#[derive(Debug, Clone)]
pub struct Plan {
    pub workspace_path: String,
}

/// Binding resolution result produced by `resolve_runtime_binding_state`.
#[derive(Debug, Clone)]
pub struct RuntimeBindingState {
    pub binding: Option<RuntimeBinding>,
    pub agent_action: String,
    pub actions_taken: Vec<String>,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub health: String,
    pub lifecycle_state: String,
    pub socket_name: Option<String>,
    pub runtime_pane_id: Option<String>,
    pub project_socket_active_pane_id: Option<String>,
}

/// Provider-payload result for a started agent runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeResult {
    pub agent_name: String,
    pub provider: String,
    pub action: String,
    pub health: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconcile_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_socket_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_socket_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_window_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_pid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

/// Overall result of starting an agent runtime.
#[derive(Debug, Clone)]
pub struct StartAgentExecution {
    pub agent_result: AgentRuntimeResult,
    pub actions_taken: Vec<String>,
    pub socket_name: Option<String>,
    pub runtime_pane_id: Option<String>,
    pub project_socket_active_pane_id: Option<String>,
}

/// Injected dependency that performs the actual pane launch / binding reuse check.
pub trait EnsureAgentRuntimeFn {
    fn call(
        &self,
        context: &Context,
        command: &Command,
        spec: &AgentSpec,
        plan: &Plan,
        binding_hint: Option<&RuntimeBinding>,
        assigned_pane_id: Option<&str>,
        style_index: usize,
        tmux_socket_path: Option<&str>,
    ) -> Result<EnsureAgentRuntimeResult, String>;
}

/// Injected dependency that decides which binding hint to pass to `ensure_agent_runtime`.
pub trait LaunchBindingHintFn {
    fn call(
        &self,
        binding: Option<&RuntimeBinding>,
        raw_binding: Option<&RuntimeBinding>,
        stale_binding: bool,
        assigned_pane_id: Option<&str>,
        tmux_socket_path: Option<&str>,
    ) -> Result<Option<RuntimeBinding>, String>;
}

/// Injected dependency that applies CCB pane identity metadata to a tmux pane.
pub trait RelabelProjectNamespacePaneFn {
    fn call(
        &self,
        binding: &RuntimeBinding,
        agent_name: &str,
        project_id: &str,
        style_index: usize,
        tmux_socket_path: Option<&str>,
        namespace_epoch: Option<i64>,
        window_name: Option<&str>,
    ) -> Result<Option<String>, String>;
}

/// Injected dependency that compares two tmux socket paths for equality.
pub trait SameTmuxSocketPathFn {
    fn call(&self, path1: Option<&str>, path2: Option<&str>) -> bool;
}

/// Arguments passed to the runtime attach service.
#[derive(Debug, Clone)]
pub struct RuntimeAttachParams {
    pub agent_name: String,
    pub workspace_path: String,
    pub backend_type: String,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub health: String,
    pub provider: String,
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<String>,
    pub terminal_backend: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: String,
    pub tmux_window_id: Option<String>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub slot_key: String,
    pub window_id: Option<String>,
    pub workspace_epoch: Option<i64>,
    pub lifecycle_state: String,
    pub managed_by: String,
    pub binding_source: String,
}

/// Runtime state record returned by the attach service.
#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub lifecycle_state: String,
    pub desired_state: String,
    pub reconcile_state: String,
    pub binding_source: String,
    pub terminal_backend: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
    pub tmux_window_id: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_state: Option<String>,
    pub runtime_pid: Option<String>,
    pub runtime_root: Option<String>,
    pub mount_attempt_id: Option<String>,
}
