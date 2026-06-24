//! Mirrors Python `lib/cli/render_runtime/mailbox_views_runtime/ack.py`.

use serde_json::Value;

use super::super::common::display_text;

/// Render an ack payload as key:value lines.
///
/// Mirrors Python `render_ack(payload)`.
pub fn render_ack(payload: &Value) -> Vec<String> {
    let mailbox = payload.get("mailbox").filter(|v| !v.is_null());
    let mut lines: Vec<String> = vec![
        "ack_status: ok".to_string(),
        format!("target: {}", str_field(payload, "target")),
        format!("agent_name: {}", str_field(payload, "agent_name")),
        format!(
            "acknowledged_inbound_event_id: {}",
            str_field(payload, "acknowledged_inbound_event_id")
        ),
        format!("message_id: {}", str_field(payload, "message_id")),
        format!("attempt_id: {}", str_field(payload, "attempt_id")),
        format!("job_id: {}", str_field(payload, "job_id")),
        format!("reply_id: {}", str_field(payload, "reply_id")),
        format!(
            "reply_from_agent: {}",
            str_field(payload, "reply_from_agent")
        ),
        format!(
            "reply_terminal_status: {}",
            str_field(payload, "reply_terminal_status")
        ),
        format!(
            "reply_notice: {}",
            payload
                .get("reply_notice")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        ),
        format!(
            "reply_notice_kind: {}",
            str_field(payload, "reply_notice_kind")
        ),
        format!(
            "reply_finished_at: {}",
            str_field(payload, "reply_finished_at")
        ),
        format!("mailbox_state: {}", mailbox_field(mailbox, "mailbox_state")),
        format!("queue_depth: {}", mailbox_field(mailbox, "queue_depth")),
        format!(
            "pending_reply_count: {}",
            mailbox_field(mailbox, "pending_reply_count")
        ),
        format!(
            "next_inbound_event_id: {}",
            str_field(payload, "next_inbound_event_id")
        ),
        format!("next_event_type: {}", str_field(payload, "next_event_type")),
    ];
    if payload
        .get("reply_last_progress_at")
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        lines.push(format!(
            "reply_last_progress_at: {}",
            str_field(payload, "reply_last_progress_at")
        ));
    }
    if payload
        .get("reply_heartbeat_silence_seconds")
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        lines.push(format!(
            "reply_heartbeat_silence_seconds: {}",
            str_field(payload, "reply_heartbeat_silence_seconds")
        ));
    }
    lines.push(format!(
        "reply: {}",
        display_text(payload.get("reply").unwrap_or(&Value::Null))
    ));
    lines
}

fn str_field(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

fn mailbox_field(mailbox: Option<&Value>, key: &str) -> String {
    mailbox
        .and_then(|m| m.get(key))
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}
