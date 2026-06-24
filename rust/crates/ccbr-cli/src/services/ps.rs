//! Mirrors Python `lib/cli/services/ps.py`.
//!
//! Builds a summary of project runtime state by reading the project config,
//! the local daemon mount state, and each agent's `AgentRuntime` record.

use serde_json::{json, Value};

use ccbr_agents::config::load_project_config;
use ccbr_agents::models::AgentRuntime;
use ccbr_agents::store::AgentRuntimeStore;
use ccbr_provider_core::session_binding::binding_status;

use crate::context::CliContext;
use crate::models::ParsedPsCommand;
use crate::services::daemon::ping_local_state;

/// Build a `ps` summary payload for the current project.
///
/// Mirrors Python `cli.services.ps.ps_summary`.
pub fn ps_summary(context: &CliContext, _command: &ParsedPsCommand) -> Value {
    let config_result = load_project_config(&context.paths).expect("load project config");
    let store = AgentRuntimeStore::new(context.paths.clone());
    let local = ping_local_state(context);

    let mut agents: Vec<Value> = Vec::new();
    let mut agent_names: Vec<String> = config_result.config.agents.keys().cloned().collect();
    agent_names.sort();

    for agent_name in agent_names {
        let spec = config_result
            .config
            .agents
            .get(&agent_name)
            .expect("agent in config")
            .clone();
        let runtime = store.load(&agent_name).ok().flatten();
        agents.push(agent_summary(context, &agent_name, &spec, runtime.as_ref()));
    }

    json!({
        "project_id": context.project.project_id,
        "ccbrd_state": local.mount_state,
        "agents": agents,
    })
}

fn agent_summary(
    context: &CliContext,
    agent_name: &str,
    spec: &ccbr_agents::models::AgentSpec,
    runtime: Option<&AgentRuntime>,
) -> Value {
    let workspace_path = workspace_path(context, agent_name, runtime);
    let runtime_ref = runtime_attr(runtime, "runtime_ref");
    let session_ref = session_ref(runtime);
    let binding_status = binding_status(
        runtime_ref.as_deref(),
        session_ref.as_deref(),
        Some(&workspace_path),
    );

    json!({
        "agent_name": agent_name,
        "provider": spec.provider,
        "runtime_mode": runtime_mode_to_str(spec.runtime_mode),
        "workspace_mode": workspace_mode_to_str(spec.workspace_mode),
        "state": runtime_enum_value(runtime, "state", "stopped"),
        "queue_depth": runtime_attr(runtime, "queue_depth").unwrap_or_else(|| "0".into()),
        "workspace_path": workspace_path,
        "runtime_ref": runtime_ref,
        "session_ref": session_ref,
        "binding_status": binding_status,
        "backend_type": runtime_attr(runtime, "backend_type")
            .unwrap_or_else(|| runtime_mode_to_str(spec.runtime_mode)),
        "binding_source": runtime_enum_value(runtime, "binding_source", "provider-session"),
        "terminal": runtime_attr(runtime, "terminal_backend"),
        "tmux_socket_name": runtime_attr(runtime, "tmux_socket_name"),
        "tmux_socket_path": runtime_attr(runtime, "tmux_socket_path"),
        "tmux_window_name": runtime_attr(runtime, "tmux_window_name"),
        "tmux_window_id": runtime_attr(runtime, "tmux_window_id"),
        "pane_id": runtime_attr(runtime, "pane_id"),
        "active_pane_id": runtime_attr(runtime, "active_pane_id"),
        "pane_title_marker": runtime_attr(runtime, "pane_title_marker"),
        "pane_state": runtime_attr(runtime, "pane_state"),
    })
}

fn workspace_path(
    context: &CliContext,
    agent_name: &str,
    runtime: Option<&AgentRuntime>,
) -> String {
    if let Some(runtime) = runtime {
        if let Some(path) = &runtime.workspace_path {
            return path.clone();
        }
    }
    context.paths.workspace_path(agent_name, None).to_string()
}

fn session_ref(runtime: Option<&AgentRuntime>) -> Option<String> {
    runtime.and_then(|r| {
        r.session_file
            .clone()
            .or_else(|| r.session_id.clone())
            .or_else(|| r.session_ref.clone())
    })
}

fn runtime_attr(runtime: Option<&AgentRuntime>, name: &str) -> Option<String> {
    runtime.and_then(|r| match name {
        "runtime_ref" => r.runtime_ref.clone(),
        "session_ref" => r.session_ref.clone(),
        "workspace_path" => r.workspace_path.clone(),
        "terminal_backend" => r.terminal_backend.clone(),
        "tmux_socket_name" => r.tmux_socket_name.clone(),
        "tmux_socket_path" => r.tmux_socket_path.clone(),
        "tmux_window_name" => r.tmux_window_name.clone(),
        "tmux_window_id" => r.tmux_window_id.clone(),
        "pane_id" => r.pane_id.clone(),
        "active_pane_id" => r.active_pane_id.clone(),
        "pane_title_marker" => r.pane_title_marker.clone(),
        "pane_state" => r.pane_state.clone(),
        "backend_type" => Some(r.backend_type.clone()).filter(|s| !s.is_empty()),
        "queue_depth" => Some(r.queue_depth.to_string()),
        _ => None,
    })
}

fn runtime_enum_value(runtime: Option<&AgentRuntime>, name: &str, default: &str) -> String {
    match name {
        "state" => runtime
            .map(|r| format!("{:?}", r.state).to_lowercase())
            .unwrap_or_else(|| default.into()),
        "binding_source" => runtime
            .map(|r| format!("{:?}", r.binding_source).to_lowercase())
            .unwrap_or_else(|| default.into()),
        _ => default.into(),
    }
}

fn runtime_mode_to_str(mode: ccbr_agents::models::RuntimeMode) -> String {
    match mode {
        ccbr_agents::models::RuntimeMode::PaneBacked => "pane-backed".into(),
        ccbr_agents::models::RuntimeMode::PtyBacked => "pty-backed".into(),
        ccbr_agents::models::RuntimeMode::Headless => "headless".into(),
    }
}

fn workspace_mode_to_str(mode: ccbr_agents::models::WorkspaceMode) -> String {
    match mode {
        ccbr_agents::models::WorkspaceMode::GitWorktree => "git-worktree".into(),
        ccbr_agents::models::WorkspaceMode::Copy => "copy".into(),
        ccbr_agents::models::WorkspaceMode::Inplace => "inplace".into(),
    }
}
