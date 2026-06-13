use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_queue(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .trim();
    if target.is_empty() {
        return Err("queue requires target".into());
    }
    let _detail = payload.get("detail").and_then(|v| v.as_bool());
    let result = app.dispatcher.queue(target);
    Ok(result)
}
