use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_cancel(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let job_id = payload
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if job_id.is_empty() {
        return Err("cancel requires job_id".into());
    }
    let receipt = app.dispatcher.cancel(job_id);
    Ok(receipt.to_record())
}
