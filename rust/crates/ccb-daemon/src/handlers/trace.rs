use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_trace(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if target.is_empty() {
        return Err("trace requires target".into());
    }
    // The mailbox trace layer understands concrete bureau ids (submission,
    // message, attempt, reply, job). For legacy agent-name or "all" targets,
    // fall back to the dispatcher's job-list trace so the CLI contract is
    // preserved.
    if target == "all"
        || !target.starts_with("sub_")
            && !target.starts_with("msg_")
            && !target.starts_with("att_")
            && !target.starts_with("rep_")
            && !target.starts_with("job_")
    {
        return Ok(app.dispatcher.trace(target));
    }
    Ok(app.mailbox_control.trace(target))
}
