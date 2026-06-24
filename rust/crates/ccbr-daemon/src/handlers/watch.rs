use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_watch(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .trim();
    if target.is_empty() {
        return Err("watch requires target".into());
    }
    let start_line = payload
        .get("start_line")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Ok(app.dispatcher.watch(target, start_line))
}
