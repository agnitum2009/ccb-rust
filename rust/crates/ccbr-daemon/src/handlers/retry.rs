use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_retry(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .trim();
    if target.is_empty() {
        return Err("retry requires target".into());
    }
    Ok(app.dispatcher.retry(target))
}
