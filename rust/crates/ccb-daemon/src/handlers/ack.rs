use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_ack(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Err("ack requires agent_name".into());
    }
    let event_id = payload.get("event_id").and_then(|v| v.as_str());
    Ok(app.dispatcher.ack_reply(agent_name, event_id))
}
