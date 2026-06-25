use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_project_restart_agent(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "reason": "missing_agent",
            "error": "restart requires exactly one agent_name",
        }));
    }
    if agent_name.to_lowercase() == "all" {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "reason": "restart_all_unsupported",
            "error": "restart all is not supported; restart exactly one configured agent",
        }));
    }

    // Force a fresh start flow for the whole project.  run_start_flow loads
    // the existing namespace, detects stale/dead panes, and recreates the
    // topology before launching provider CLIs again.  We restart all agents
    // rather than just the requested one because the layout namespace must
    // remain consistent across all panes after an external kill-pane.
    let auto_permission = payload
        .get("auto_permission")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let config_windows = app.current_config.as_ref().and_then(|c| c.windows.clone());
    let all_agents: Vec<String> = app.agent_names();
    if !all_agents.contains(&agent_name.to_string()) {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "agent_name": agent_name,
            "reason": "agent not configured",
            "recreate_reason": "manual_restart_agent",
        }));
    }
    let result = app.run_start_flow(
        &all_agents,
        false, // restore=false: do not restore sessions, respawn fresh
        auto_permission,
        config_windows,
    )?;

    let agent_result = result
        .agent_results
        .into_iter()
        .find(|a| a.agent_name == agent_name)
        .ok_or_else(|| format!("restart produced no result for agent {}", agent_name))?;

    if agent_result.status == "failed" {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "agent_name": agent_name,
            "reason": agent_result.reason.unwrap_or_else(|| "unknown".to_string()),
            "recreate_reason": "manual_restart_agent",
        }));
    }

    Ok(json!({
        "status": "ok",
        "restart_status": "ok",
        "agent_name": agent_name,
        "pane_id": agent_result.pane_id,
        "recreate_reason": "manual_restart_agent",
    }))
}

pub fn handle_project_restart_panes(_app: &mut CcbdApp, _payload: &Value) -> Result<Value, String> {
    Ok(json!({
        "status": "scheduled",
        "restart_mode": "in_place",
        "recreate_reason": "manual_restart_panes",
    }))
}
