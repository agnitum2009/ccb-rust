use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_project_focus_window(_app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let window = payload.get("window").and_then(|v| v.as_str()).unwrap_or("");
    let namespace_epoch = payload.get("namespace_epoch").and_then(|v| v.as_u64());
    Ok(json!({
        "status": "ok",
        "window": window,
        "namespace_epoch": namespace_epoch,
    }))
}

pub fn handle_project_focus_agent(_app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent = payload.get("agent").and_then(|v| v.as_str()).unwrap_or("");
    let namespace_epoch = payload.get("namespace_epoch").and_then(|v| v.as_u64());
    Ok(json!({
        "status": "ok",
        "agent": agent,
        "namespace_epoch": namespace_epoch,
    }))
}
