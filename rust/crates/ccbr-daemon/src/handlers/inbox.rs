use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_inbox(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Err("inbox requires agent_name".into());
    }
    let detail = payload.get("detail").and_then(|v| v.as_bool());
    Ok(app.mailbox_control.inbox(agent_name, detail))
}
