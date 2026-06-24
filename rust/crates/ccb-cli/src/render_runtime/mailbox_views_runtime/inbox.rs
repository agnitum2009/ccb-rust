//! Mirrors Python `lib/cli/render_runtime/mailbox_views_runtime/inbox.py`.

use serde_json::Value;

use super::super::common::{display_text, observer_status_is_terminal, render_observer_notice};

/// Render an inbox payload as key:value lines.
///
/// Mirrors Python `render_inbox(payload)`.
pub fn render_inbox(payload: &Value) -> Vec<String> {
    let agent = payload.get("agent").filter(|v| !v.is_null());
    let head = payload.get("head").filter(|v| !v.is_null());
    let terminal = observer_status_is_terminal(
        head.and_then(|h| h.get("reply_terminal_status"))
            .unwrap_or(&Value::Null),
    );
    let status = if str_field(payload, "summary_status") == "ok" {
        "ok"
    } else {
        "degraded"
    };

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("inbox_status: {}", status));
    lines.extend(render_observer_notice(
        "inbox",
        terminal,
        "supplementary_snapshot",
    ));
    lines.push(format!("target: {}", str_field(payload, "target")));
    lines.push(format!("agent_name: {}", nested_field(agent, "agent_name")));
    lines.push(format!("mailbox_id: {}", nested_field(agent, "mailbox_id")));
    lines.push(format!(
        "summary_status: {}",
        str_field(payload, "summary_status")
    ));
    lines.push(format!(
        "mailbox_state: {}",
        nested_field(agent, "mailbox_state")
    ));
    lines.push(format!(
        "lease_version: {}",
        nested_field(agent, "lease_version")
    ));
    lines.push(format!(
        "queue_depth: {}",
        nested_field(agent, "queue_depth")
    ));
    lines.push(format!(
        "pending_reply_count: {}",
        nested_field(agent, "pending_reply_count")
    ));
    lines.push(format!(
        "active_inbound_event_id: {}",
        nested_field(agent, "active_inbound_event_id")
    ));
    lines.push(format!("item_count: {}", str_field(payload, "item_count")));
    lines.push(format!(
        "head_inbound_event_id: {}",
        nested_field(head, "inbound_event_id")
    ));
    lines.push(format!(
        "head_event_type: {}",
        nested_field(head, "event_type")
    ));
    lines.push(format!("head_status: {}", nested_field(head, "status")));

    if payload
        .get("summary_error")
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        lines.push(format!(
            "summary_error: {}",
            str_field(payload, "summary_error")
        ));
    }
    match str_field(payload, "summary_status").as_str() {
        "missing" => lines.push(
            "summary_notice: persisted mailbox summary is missing; routine observer view is degraded; use `ccb doctor` or wait for maintenance refresh".to_string(),
        ),
        "error" => lines.push(
            "summary_notice: persisted mailbox summary is unreadable; routine observer view is degraded; use `ccb doctor` for diagnostics".to_string(),
        ),
        _ => {}
    }

    if head
        .and_then(|h| h.get("reply_id"))
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        lines.push(format!("head_reply_id: {}", nested_field(head, "reply_id")));
        lines.push(format!(
            "head_reply_from_agent: {}",
            nested_field(head, "source_actor")
        ));
        lines.push(format!(
            "head_reply_terminal_status: {}",
            nested_field(head, "reply_terminal_status")
        ));
        lines.push(format!(
            "head_reply_notice: {}",
            head.and_then(|h| h.get("reply_notice"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        ));
        lines.push(format!(
            "head_reply_notice_kind: {}",
            nested_field(head, "reply_notice_kind")
        ));
        lines.push(format!(
            "head_reply_job_id: {}",
            nested_field(head, "job_id")
        ));
        lines.push(format!(
            "head_reply_finished_at: {}",
            nested_field(head, "reply_finished_at")
        ));
        if head
            .and_then(|h| h.get("reply_last_progress_at"))
            .map(|v| !v.is_null())
            .unwrap_or(false)
        {
            lines.push(format!(
                "head_reply_last_progress_at: {}",
                nested_field(head, "reply_last_progress_at")
            ));
        }
        if head
            .and_then(|h| h.get("reply_heartbeat_silence_seconds"))
            .map(|v| !v.is_null())
            .unwrap_or(false)
        {
            lines.push(format!(
                "head_reply_heartbeat_silence_seconds: {}",
                nested_field(head, "reply_heartbeat_silence_seconds")
            ));
        }
    }
    if head
        .and_then(|h| h.get("reply"))
        .map(|v| !v.is_null())
        .unwrap_or(false)
    {
        lines.push(format!(
            "reply: {}",
            display_text(head.and_then(|h| h.get("reply")).unwrap_or(&Value::Null))
        ));
    }

    let items = payload.get("items");
    let item_count = payload.get("item_count").and_then(|v| v.as_i64());
    if items
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(false)
        && item_count.is_some()
        && item_count != Some(0)
    {
        lines.push("inbox_details: omitted; rerun with `ccb pend --inbox --detail <agent>` or `ccb inbox --detail <agent>` for inbox-item detail".to_string());
        return lines;
    }
    if let Some(Value::Array(arr)) = items {
        for item in arr {
            let mut parts: Vec<String> = vec![
                "inbox_item:".to_string(),
                format!("pos={}", nested_field(Some(item), "position")),
                format!("event={}", nested_field(Some(item), "inbound_event_id")),
                format!("type={}", nested_field(Some(item), "event_type")),
                format!("status={}", nested_field(Some(item), "status")),
                format!("priority={}", nested_field(Some(item), "priority")),
                format!("message={}", nested_field(Some(item), "message_id")),
                format!("attempt={}", nested_field(Some(item), "attempt_id")),
                format!("job={}", nested_field(Some(item), "job_id")),
                format!("from={}", nested_field(Some(item), "source_actor")),
            ];
            if item.get("reply_id").map(|v| !v.is_null()).unwrap_or(false) {
                parts.push(format!("reply={}", nested_field(Some(item), "reply_id")));
                parts.push(format!(
                    "terminal={}",
                    nested_field(Some(item), "reply_terminal_status")
                ));
                parts.push(format!(
                    "notice={}",
                    item.get("reply_notice")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                ));
                parts.push(format!(
                    "kind={}",
                    nested_field(Some(item), "reply_notice_kind")
                ));
                parts.push(format!(
                    "control_job={}",
                    nested_field(Some(item), "job_id")
                ));
                parts.push(format!(
                    "preview={}",
                    nested_field(Some(item), "reply_preview")
                ));
            }
            lines.push(parts.join(" "));
        }
    }
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

fn nested_field(parent: Option<&Value>, key: &str) -> String {
    parent
        .and_then(|p| p.get(key))
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}
