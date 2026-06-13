use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_restore(_app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Err("restore requires agent_name".into());
    }
    Ok(json!({
        "agent_name": agent_name,
        "status": "restored",
    }))
}
