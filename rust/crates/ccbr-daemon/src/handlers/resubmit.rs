use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_resubmit(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let message_id = payload
        .get("message_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if message_id.is_empty() {
        return Err("resubmit requires message_id".into());
    }
    Ok(app.dispatcher.resubmit(message_id))
}
