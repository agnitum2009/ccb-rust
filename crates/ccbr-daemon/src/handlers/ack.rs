use serde_json::{json, Value};

use crate::app::CcbdApp;

pub fn handle_ack(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Err("ack requires agent_name".into());
    }
    let event_id = payload.get("event_id").and_then(|v| v.as_str());

    // Guard against the mailbox control layer panicking when there is no
    // ackable reply head. If the head is missing, the requested event_id does
    // not match it, or the head is not a task_reply, return a graceful no-op.
    let head = app.mailbox_control.mailbox_head(agent_name);
    let head_event_id = head
        .get("head")
        .and_then(|h| h.as_object())
        .and_then(|o| o.get("inbound_event_id"))
        .and_then(|v| v.as_str());
    let head_event_type = head
        .get("head")
        .and_then(|h| h.as_object())
        .and_then(|o| o.get("event_type"))
        .and_then(|v| v.as_str());

    if head_event_id.is_none() {
        return Ok(json!({
            "agent_name": agent_name,
            "inbound_event_id": event_id,
            "status": "acked",
            "note": "no reply head to acknowledge",
        }));
    }

    if let Some(requested) = event_id {
        if head_event_id != Some(requested) {
            return Ok(json!({
                "agent_name": agent_name,
                "inbound_event_id": event_id,
                "status": "acked",
                "note": "requested event is not the current head",
            }));
        }
    }

    if head_event_type == Some("task_reply") {
        Ok(app.mailbox_control.ack_reply(agent_name, event_id))
    } else {
        Ok(json!({
            "agent_name": agent_name,
            "inbound_event_id": event_id,
            "status": "acked",
            "note": "current head is not a reply",
        }))
    }
}
