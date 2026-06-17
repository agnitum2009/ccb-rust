//! Mirrors Python `lib/ccbd/services/runtime_runtime/attach.py`.
//! 1:1 file alignment stub.

use ccb_agents::models::{AgentRuntime, RuntimeBindingSource};

/// Attach runtime to agent registry
pub fn attach_runtime(
    registry: &dyn Registry,
    project_id: &str,
    agent_name: &str,
    workspace_path: &str,
    backend_type: &str,
    timestamp: &str,
) -> Result<AgentRuntime, String> {
    let spec = registry.spec_for(agent_name)?;
    let _existing = registry.get(agent_name);

    // Create new runtime with minimal fields
    let runtime = AgentRuntime {
        agent_name: agent_name.to_string(),
        state: ccb_agents::models::AgentState::Idle,
        project_id: project_id.to_string(),
        backend_type: backend_type.to_string(),
        workspace_path: Some(workspace_path.to_string()),
        health: "unknown".to_string(),
        provider: Some(spec.provider.clone()),
        binding_source: RuntimeBindingSource::ProviderSession,
        binding_generation: 1,
        runtime_generation: Some(1),
        managed_by: "ccbd".to_string(),
        started_at: Some(timestamp.to_string()),
        last_seen_at: Some(timestamp.to_string()),
        queue_depth: 0,
        pid: None,
        socket_path: None,
        runtime_ref: None,
        session_ref: None,
        runtime_root: None,
        runtime_pid: None,
        terminal_backend: None,
        pane_id: None,
        active_pane_id: None,
        pane_title_marker: None,
        pane_state: None,
        tmux_socket_name: None,
        tmux_socket_path: None,
        tmux_window_name: None,
        tmux_window_id: None,
        session_file: None,
        session_id: None,
        slot_key: None,
        window_id: None,
        workspace_epoch: None,
        lifecycle_state: None,
        daemon_generation: None,
        desired_state: None,
        reconcile_state: None,
        restart_count: 0,
        last_reconcile_at: None,
        last_failure_reason: None,
        mount_attempt_id: None,
    };

    Ok(runtime)
}

// Simplified trait for registry interaction
pub trait Registry {
    fn spec_for(&self, agent_name: &str) -> Result<AgentSpec, String>;
    fn get(&self, agent_name: &str) -> Option<AgentRuntime>;
    fn upsert(&self, runtime: AgentRuntime) -> Result<AgentRuntime, String>;
}

#[derive(Clone, Debug)]
pub struct AgentSpec {
    pub name: String,
    pub provider: String,
}
