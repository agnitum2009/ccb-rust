//! Mirrors Python `lib/cli/render_runtime/mailbox_views_runtime/trace.py`.

use serde_json::Value;

/// Render a trace payload (header + submission/messages/attempts/replies/events/jobs).
///
/// Mirrors Python `render_trace(payload)`.
pub fn render_trace(payload: &Value) -> Vec<String> {
    let mut lines = trace_header_lines(payload);
    if let Some(submission) = payload.get("submission").filter(|v| v.is_object()) {
        lines.push(submission_line(submission));
    }
    for_each_item(payload, "messages", |m| lines.push(message_line(m)));
    for_each_item(payload, "attempts", |a| lines.push(attempt_line(a)));
    for_each_item(payload, "replies", |r| lines.push(reply_line(r)));
    for_each_item(payload, "events", |e| lines.push(event_line(e)));
    for_each_item(payload, "jobs", |j| lines.push(job_line(j)));
    lines
}

fn trace_header_lines(payload: &Value) -> Vec<String> {
    let mut lines = vec!["trace_status: ok".to_string()];
    for (name, key) in [
        ("target", "target"),
        ("resolved_kind", "resolved_kind"),
        ("submission_id", "submission_id"),
        ("message_id", "message_id"),
        ("attempt_id", "attempt_id"),
        ("reply_id", "reply_id"),
        ("job_id", "job_id"),
        ("message_count", "message_count"),
        ("attempt_count", "attempt_count"),
        ("reply_count", "reply_count"),
        ("event_count", "event_count"),
        ("job_count", "job_count"),
    ] {
        lines.push(format!("{}: {}", name, field(payload, key)));
    }
    lines
}

fn submission_line(submission: &Value) -> String {
    let job_count = submission
        .get("job_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    format!(
        "submission: id={} from={} scope={} task={} jobs={} created={} updated={}",
        field(submission, "submission_id"),
        field(submission, "from_actor"),
        field(submission, "target_scope"),
        field(submission, "task_id"),
        job_count,
        field(submission, "created_at"),
        field(submission, "updated_at"),
    )
}

fn message_line(message: &Value) -> String {
    let targets = message
        .get("target_agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        "message: id={} submission={} origin={} from={} scope={} targets={} class={} state={} priority={} created={} updated={}",
        field(message, "message_id"),
        field(message, "submission_id"),
        field(message, "origin_message_id"),
        field(message, "from_actor"),
        field(message, "target_scope"),
        targets,
        field(message, "message_class"),
        field(message, "message_state"),
        field(message, "priority"),
        field(message, "created_at"),
        field(message, "updated_at"),
    )
}

fn attempt_line(attempt: &Value) -> String {
    format!(
        "attempt: id={} message={} agent={} provider={} job={} retry={} state={} started={} updated={}",
        field(attempt, "attempt_id"),
        field(attempt, "message_id"),
        field(attempt, "agent_name"),
        field(attempt, "provider"),
        field(attempt, "job_id"),
        field(attempt, "retry_index"),
        field(attempt, "attempt_state"),
        field(attempt, "started_at"),
        field(attempt, "updated_at"),
    )
}

fn reply_line(reply: &Value) -> String {
    let notice = reply
        .get("notice")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    format!(
        "reply: id={} message={} attempt={} agent={} terminal={} size={} notice={} kind={} reason={} finished={} preview={}",
        field(reply, "reply_id"),
        field(reply, "message_id"),
        field(reply, "attempt_id"),
        field(reply, "agent_name"),
        field(reply, "terminal_status"),
        field(reply, "reply_size"),
        notice,
        field(reply, "notice_kind"),
        field(reply, "reason"),
        field(reply, "finished_at"),
        field(reply, "reply_preview"),
    )
}

fn event_line(event: &Value) -> String {
    let mailbox_active = event
        .get("mailbox_active")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    format!(
        "event: id={} agent={} type={} status={} mailbox_state={} active={} message={} attempt={} created={} finished={}",
        field(event, "inbound_event_id"),
        field(event, "agent_name"),
        field(event, "event_type"),
        field(event, "status"),
        field(event, "mailbox_state"),
        mailbox_active,
        field(event, "message_id"),
        field(event, "attempt_id"),
        field(event, "created_at"),
        field(event, "finished_at"),
    )
}

fn job_line(job: &Value) -> String {
    format!(
        "job: id={} agent={} provider={} status={} submission={} created={} updated={}",
        field(job, "job_id"),
        field(job, "agent_name"),
        field(job, "provider"),
        field(job, "status"),
        field(job, "submission_id"),
        field(job, "created_at"),
        field(job, "updated_at"),
    )
}

fn for_each_item<F>(payload: &Value, key: &str, mut f: F)
where
    F: FnMut(&Value),
{
    if let Some(Value::Array(arr)) = payload.get(key) {
        for item in arr {
            f(item);
        }
    }
}

fn field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(|v| match v {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}
