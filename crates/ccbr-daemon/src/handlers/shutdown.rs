use serde_json::{json, Value};

use crate::app::CcbdApp;

pub fn handle_shutdown(app: &mut CcbdApp, _payload: &Value) -> Result<Value, String> {
    app.request_shutdown();
    Ok(json!({
        "status": "ok",
        "trigger": "shutdown",
        "reason": "shutdown",
    }))
}
