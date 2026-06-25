use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::services::registry::AgentRuntimeEntry;

pub fn handle_ping(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("ccbd")
        .trim()
        .to_lowercase();

    let inspection = app.health_monitor.daemon_health();
    let project_id = app.project_id().to_string();

    if target.is_empty() || target == "ccbd" {
        return Ok(build_ccbd_payload(app, &project_id, &inspection));
    }

    if target == "all" {
        let agents: Vec<Value> = app
            .registry
            .all_entries()
            .into_iter()
            .map(|e| build_agent_payload(&project_id, e, &inspection))
            .collect();
        return Ok(json!({
            "pong": true,
            "target": "all",
            "project_id": project_id,
            "ccbd_state": ccbd_state(app, &inspection),
            "agents": agents,
        }));
    }

    let agent = app
        .registry
        .get(&target)
        .ok_or_else(|| format!("unknown agent: {target}"))?;
    Ok(build_agent_payload(&project_id, agent, &inspection))
}

fn build_ccbd_payload(
    app: &CcbdApp,
    project_id: &str,
    inspection: &crate::services::health::HealthInspection,
) -> Value {
    let known_agents: Vec<String> = app
        .current_config
        .as_ref()
        .map(|c| c.agents.keys().cloned().collect())
        .unwrap_or_else(|| {
            app.registry
                .all_entries()
                .into_iter()
                .map(|e| e.agent_name.clone())
                .collect()
        });

    let namespace = app.project_namespace.load();
    let namespace_summary = namespace
        .map(|ns| ns.to_record())
        .unwrap_or_else(|| json!({"mounted": false}));
    let namespace_workspace_window_name = namespace
        .and_then(|ns| ns.windows.first())
        .map(|w| w.name.clone())
        .unwrap_or_else(|| "workspace".to_string());
    let namespace_ui_attachable = namespace.map(|ns| {
        !ns.tmux_socket_path.is_empty()
            && !ns.tmux_session_name.is_empty()
    }).unwrap_or(false);
    let namespace_tmux_socket_path = namespace
        .map(|ns| ns.tmux_socket_path.clone())
        .unwrap_or_default();
    let namespace_tmux_session_name = namespace
        .map(|ns| ns.tmux_session_name.clone())
        .unwrap_or_default();

    let start_policy_summary = app
        .start_policy_store
        .load()
        .ok()
        .flatten()
        .map(|p| serde_json::json!(p))
        .unwrap_or_else(|| json!({"recovery_restore": false, "auto_permission": false}));

    json!({
        "pong": true,
        "target": "ccbd",
        "status": "ok",
        "project_id": project_id,
        "mount_state": ccbd_state(app, inspection),
        "desired_state": Value::Null,
        "health": inspection.health(),
        "generation": inspection.generation,
        "socket_path": app.socket_path(),
        "tmux_socket_path": app.tmux_socket_path(),
        "known_agents": known_agents,
        "config_signature": project_id,
        "namespace_summary": namespace_summary,
        "namespace_tmux_socket_path": namespace_tmux_socket_path,
        "namespace_tmux_session_name": namespace_tmux_session_name,
        "namespace_workspace_window_name": namespace_workspace_window_name,
        "namespace_ui_attachable": namespace_ui_attachable,
        "start_policy_summary": start_policy_summary,
        "diagnostics": {
            "pid_alive": inspection.daemon_alive,
            "socket_connectable": inspection.socket_connectable,
            "heartbeat_fresh": inspection.socket_connectable,
            "takeover_allowed": false,
            "reason": Value::Null,
            "startup_id": Value::Null,
            "startup_stage": Value::Null,
            "last_progress_at": Value::Null,
            "startup_deadline_at": Value::Null,
            "last_failure_reason": Value::Null,
            "shutdown_intent": if app.is_shutdown_requested() { Value::String("requested".into()) } else { Value::Null },
            "agent_count": inspection.agent_count,
            "healthy_count": inspection.healthy_count,
            "degraded_count": inspection.degraded_count,
            "failed_count": inspection.failed_count,
        },
        "health_record": inspection.to_record(),
    })
}

fn build_agent_payload(
    project_id: &str,
    runtime: &AgentRuntimeEntry,
    inspection: &crate::services::health::HealthInspection,
) -> Value {
    json!({
        "pong": true,
        "target": runtime.agent_name,
        "project_id": project_id,
        "agent_name": runtime.agent_name,
        "provider": runtime.provider,
        "mount_state": agent_mount_state(runtime),
        "runtime_state": runtime.state,
        "health": runtime.health,
        "pane_id": runtime.pane_id,
        "workspace_path": runtime.workspace_path,
        "diagnostics": {
            "ccbd_generation": inspection.generation,
            "last_heartbeat_at": Value::Null,
            "desired_state": Value::Null,
        },
    })
}

fn ccbd_state(
    app: &CcbdApp,
    inspection: &crate::services::health::HealthInspection,
) -> &'static str {
    if app.is_shutdown_requested() {
        return "stopping";
    }
    if inspection.daemon_alive {
        "running"
    } else {
        "unmounted"
    }
}

fn agent_mount_state(runtime: &AgentRuntimeEntry) -> &'static str {
    match runtime.state.as_str() {
        "starting" => "starting",
        "failed" => "failed",
        "stopped" => "unmounted",
        _ => "mounted",
    }
}
