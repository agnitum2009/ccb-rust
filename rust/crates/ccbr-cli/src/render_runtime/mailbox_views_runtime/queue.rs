//! Mirrors Python `lib/cli/render_runtime/mailbox_views_runtime/queue.py`.

use serde_json::Value;

use super::super::common::render_observer_notice;

/// Render a queue payload (aggregate `all` view or single-agent detail view).
///
/// Mirrors Python `render_queue(payload)`.
pub fn render_queue(payload: &Value) -> Vec<String> {
    let target = field(payload, "target");
    let mut status = "ok".to_string();
    if target != "all" {
        let agent_status = payload
            .get("agent")
            .and_then(|a| a.get("summary_status"))
            .map(field_value)
            .unwrap_or_default();
        if agent_status != "ok" {
            status = "degraded".to_string();
        }
    }

    let mut lines = vec![format!("queue_status: {}", status)];
    lines.extend(render_observer_notice(
        "queue",
        false,
        "supplementary_snapshot",
    ));
    lines.push(format!("target: {}", target));

    if target == "all" {
        lines.push(format!("agent_count: {}", field(payload, "agent_count")));
        lines.push(format!(
            "queued_agent_count: {}",
            field(payload, "queued_agent_count")
        ));
        lines.push(format!(
            "active_agent_count: {}",
            field(payload, "active_agent_count")
        ));
        lines.push(format!(
            "total_queue_depth: {}",
            field(payload, "total_queue_depth")
        ));
        lines.push(format!(
            "total_pending_reply_count: {}",
            field(payload, "total_pending_reply_count")
        ));
        if let Some(Value::Array(agents)) = payload.get("agents") {
            for agent in agents {
                lines.push(format!(
                    "queue_agent: name={} runtime_state={} runtime_health={} state={} depth={} pending_replies={} summary_status={}",
                    field(agent, "agent_name"),
                    field(agent, "runtime_state"),
                    field(agent, "runtime_health"),
                    field(agent, "mailbox_state"),
                    field(agent, "queue_depth"),
                    field(agent, "pending_reply_count"),
                    field(agent, "summary_status"),
                ));
            }
        }
        return lines;
    }

    let agent = payload.get("agent");
    let agent_field = |key: &str| field(agent.unwrap_or(&Value::Null), key);
    lines.push(format!("agent_name: {}", agent_field("agent_name")));
    lines.push(format!("mailbox_id: {}", agent_field("mailbox_id")));
    lines.push(format!("summary_status: {}", agent_field("summary_status")));
    lines.push(format!("mailbox_state: {}", agent_field("mailbox_state")));
    lines.push(format!("runtime_state: {}", agent_field("runtime_state")));
    lines.push(format!("runtime_health: {}", agent_field("runtime_health")));
    lines.push(format!("lease_version: {}", agent_field("lease_version")));
    lines.push(format!("queue_depth: {}", agent_field("queue_depth")));
    lines.push(format!(
        "pending_reply_count: {}",
        agent_field("pending_reply_count")
    ));
    lines.push(format!(
        "active_inbound_event_id: {}",
        agent_field("active_inbound_event_id")
    ));
    lines.push(format!(
        "last_inbound_started_at: {}",
        agent_field("last_inbound_started_at")
    ));
    lines.push(format!(
        "last_inbound_finished_at: {}",
        agent_field("last_inbound_finished_at")
    ));

    if let Some(err) = agent.and_then(|a| a.get("summary_error")) {
        if !err.is_null() {
            lines.push(format!("summary_error: {}", field_value(err)));
        }
    }
    match agent
        .and_then(|a| a.get("summary_status"))
        .and_then(|v| v.as_str())
    {
        Some("missing") => lines.push(
            "summary_notice: persisted mailbox summary is missing; routine observer view is degraded; use `ccb doctor` or wait for maintenance refresh".to_string(),
        ),
        Some("error") => lines.push(
            "summary_notice: persisted mailbox summary is unreadable; routine observer view is degraded; use `ccb doctor` for diagnostics".to_string(),
        ),
        _ => {}
    }

    if let Some(active) = agent
        .and_then(|a| a.get("active"))
        .filter(|v| v.is_object())
    {
        lines.push(format!(
            "queue_active: event={} type={} status={} message={} attempt={} job={}",
            field(active, "inbound_event_id"),
            field(active, "event_type"),
            field(active, "status"),
            field(active, "message_id"),
            field(active, "attempt_id"),
            field(active, "job_id"),
        ));
    }

    let queued_events = agent.and_then(|a| a.get("queued_events"));
    if queued_events.map(|v| v.is_null()).unwrap_or(true) {
        lines.push("queue_details: omitted; rerun with `ccb pend --queue --detail <agent>` or `ccb queue --detail <agent>` for queued-event detail".to_string());
        return lines;
    }
    if let Some(Value::Array(events)) = queued_events {
        for event in events {
            lines.push(format!(
                "queue_event: pos={} event={} type={} status={} priority={} message={} attempt={} job={}",
                field(event, "position"),
                field(event, "inbound_event_id"),
                field(event, "event_type"),
                field(event, "status"),
                field(event, "priority"),
                field(event, "message_id"),
                field(event, "attempt_id"),
                field(event, "job_id"),
            ));
        }
    }
    lines
}

fn field(value: &Value, key: &str) -> String {
    field_value(value.get(key).unwrap_or(&Value::Null))
}

fn field_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
