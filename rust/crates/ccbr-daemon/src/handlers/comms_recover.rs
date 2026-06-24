use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_comms_recover(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    Ok(app.dispatcher.comms_recover(payload))
}
