//! Mirrors Python `lib/cli/render_runtime/mailbox_views_runtime/job.py`.

use serde_json::Value;

use super::super::common::{display_text, observer_status_is_terminal, render_observer_notice};

const JOB_STATE_KEYS: &[&str] = &[
    "job_id",
    "agent_name",
    "target_kind",
    "target_name",
    "provider",
    "provider_instance",
    "status",
    "reply",
    "completion_reason",
    "completion_confidence",
    "updated_at",
];

/// Render the core job-state fields as `key: value` lines.
///
/// Mirrors Python `render_job_state(payload)`.
pub fn render_job_state(payload: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for &key in JOB_STATE_KEYS {
        let value = payload.get(key).unwrap_or(&Value::Null);
        let rendered = if key == "reply" {
            display_text(value)
        } else {
            field_value(value)
        };
        lines.push(format!("{}: {}", key, rendered));
    }
    lines
}

/// Render a pend view: job state + observer notice + optional mailbox reply block.
///
/// Mirrors Python `render_pend(payload)`.
pub fn render_pend(payload: &Value) -> Vec<String> {
    let mut lines = render_job_state(payload);
    let mut terminal = observer_status_is_terminal(payload.get("status").unwrap_or(&Value::Null));
    if let Some(mailbox_status) = payload.get("mailbox_reply_terminal_status") {
        if !mailbox_status.is_null() {
            terminal = observer_status_is_terminal(mailbox_status);
        }
    }
    lines.extend(render_observer_notice(
        "pend",
        terminal,
        "supplementary_snapshot",
    ));

    if let Some(value) = payload.get("mailbox_summary_status") {
        if !value.is_null() {
            lines.push(format!("mailbox_summary_status: {}", field_value(value)));
        }
    }
    if let Some(value) = payload.get("mailbox_summary_error") {
        if !value.is_null() {
            lines.push(format!("mailbox_summary_error: {}", field_value(value)));
        }
    }
    if let Some(value) = payload.get("mailbox_reply_ready") {
        if !value.is_null() {
            lines.push(format!(
                "mailbox_reply_ready: {}",
                value.as_bool().unwrap_or(false)
            ));
            lines.push(format!(
                "mailbox_reply_id: {}",
                field(payload, "mailbox_reply_id")
            ));
            lines.push(format!(
                "mailbox_reply_from_agent: {}",
                field(payload, "mailbox_reply_from_agent")
            ));
            lines.push(format!(
                "mailbox_reply_terminal_status: {}",
                field(payload, "mailbox_reply_terminal_status")
            ));
            lines.push(format!(
                "mailbox_reply_notice: {}",
                payload
                    .get("mailbox_reply_notice")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            ));
            lines.push(format!(
                "mailbox_reply_notice_kind: {}",
                field(payload, "mailbox_reply_notice_kind")
            ));
            lines.push(format!(
                "mailbox_reply_job_id: {}",
                field(payload, "mailbox_reply_job_id")
            ));
            lines.push(format!(
                "mailbox_reply_finished_at: {}",
                field(payload, "mailbox_reply_finished_at")
            ));
            if let Some(progress) = payload.get("mailbox_reply_last_progress_at") {
                if !progress.is_null() {
                    lines.push(format!(
                        "mailbox_reply_last_progress_at: {}",
                        field_value(progress)
                    ));
                }
            }
            if let Some(silence) = payload.get("mailbox_reply_heartbeat_silence_seconds") {
                if !silence.is_null() {
                    lines.push(format!(
                        "mailbox_reply_heartbeat_silence_seconds: {}",
                        field_value(silence)
                    ));
                }
            }
            if let Some(reply) = payload.get("mailbox_reply") {
                if !reply.is_null() {
                    lines.push(format!("mailbox_reply: {}", display_text(reply)));
                }
            }
        }
    }
    lines
}

fn field(payload: &Value, key: &str) -> String {
    field_value(payload.get(key).unwrap_or(&Value::Null))
}

fn field_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
