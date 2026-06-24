use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::services::runtime::RuntimeAttachParams;

pub fn handle_attach(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let workspace_path = payload
        .get("workspace_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let backend_type = payload
        .get("backend_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if agent_name.is_empty() {
        return Err("agent_name required".into());
    }
    if workspace_path.is_empty() {
        return Err("workspace_path required".into());
    }
    if backend_type.is_empty() {
        return Err("backend_type required".into());
    }

    let params = RuntimeAttachParams {
        agent_name: agent_name.to_string(),
        workspace_path: workspace_path.to_string(),
        backend_type: backend_type.to_string(),
        pid: payload
            .get("pid")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        runtime_ref: payload
            .get("runtime_ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        session_ref: payload
            .get("session_ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        health: payload
            .get("health")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        provider: payload
            .get("provider")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        pane_id: payload
            .get("pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        active_pane_id: payload
            .get("active_pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        session_file: payload
            .get("session_file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        session_id: payload
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        binding_source: payload
            .get("binding_source")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    };

    let entry = app.runtime_service.attach(&mut app.registry, params);
    Ok(json!({
        "agent_name": entry.agent_name,
        "workspace_path": workspace_path,
        "backend_type": backend_type,
        "pane_id": entry.pane_id,
        "status": "attached",
    }))
}
