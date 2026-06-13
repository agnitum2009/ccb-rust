use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_mailbox_head(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Err("mailbox_head requires agent_name".into());
    }
    Ok(app.dispatcher.mailbox_head(agent_name))
}
