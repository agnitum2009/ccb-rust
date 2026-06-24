use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_project_restart_agent(_app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
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
    Ok(json!({
        "status": "ok",
        "restart_status": "ok",
        "agent_name": agent_name,
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
