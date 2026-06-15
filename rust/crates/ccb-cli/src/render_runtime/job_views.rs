//! Mirrors Python `lib/cli/render_runtime/job_views.py`.

use serde_json::Value;

use super::common::{display_text, render_mapping, render_observer_notice};

/// Render an ask submission summary (single or multi-job).
///
/// Mirrors Python `render_ask(summary)`.
pub fn render_ask(summary: &Value) -> Vec<String> {
    let jobs = summary.get("jobs").and_then(|v| v.as_array());
    let jobs: Vec<&Value> = jobs.map(|a| a.iter().collect()).unwrap_or_default();
    if jobs.len() == 1 {
        let job = jobs[0];
        let target = job_target(job);
        let job_id = field(job, "job_id");
        return vec![
            format!("accepted job={} target={}", job_id, target),
            format!("[CCB_ASYNC_SUBMITTED job={} target={}]", job_id, target),
        ];
    }
    let rendered = jobs
        .iter()
        .map(|job| format!("{}@{}", field(job, "job_id"), job_target(job)))
        .collect::<Vec<_>>()
        .join(",");
    vec![
        format!("accepted jobs={}", rendered),
        format!("[CCB_ASYNC_SUBMITTED jobs={}]", rendered),
    ]
}

/// Render a resubmit summary.
///
/// Mirrors Python `render_resubmit(summary)`.
pub fn render_resubmit(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        "resubmit_status: accepted".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("original_message_id: {}", field(summary, "original_message_id")),
        format!("message_id: {}", field(summary, "message_id")),
        format!("submission_id: {}", field(summary, "submission_id")),
    ];
    if let Some(Value::Array(jobs)) = summary.get("jobs") {
        for job in jobs {
            let target = job_target(job);
            lines.push(format!(
                "job: {} {} {}",
                field(job, "job_id"),
                target,
                field(job, "status")
            ));
        }
    }
    lines
}

/// Render a retry summary.
///
/// Mirrors Python `render_retry(summary)`.
pub fn render_retry(summary: &Value) -> Vec<String> {
    vec![
        "retry_status: accepted".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("target: {}", field(summary, "target")),
        format!("message_id: {}", field(summary, "message_id")),
        format!("original_attempt_id: {}", field(summary, "original_attempt_id")),
        format!("attempt_id: {}", field(summary, "attempt_id")),
        format!("job_id: {}", field(summary, "job_id")),
        format!("agent_name: {}", field(summary, "agent_name")),
        format!("status: {}", field(summary, "status")),
    ]
}

/// Render a wait summary.
///
/// Mirrors Python `render_wait(summary)`.
pub fn render_wait(summary: &Value) -> Vec<String> {
    let wait_status = field_or(summary, "wait_status", "satisfied");
    let received_count = field(summary, "received_count");
    let terminal_count = field_or(summary, "terminal_count", &received_count);
    let notice_count = field_or(summary, "notice_count", "0");
    let waited_s = summary
        .get("waited_s")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let mut lines = vec![
        format!("wait_status: {}", wait_status),
        format!("project_id: {}", field(summary, "project_id")),
        format!("mode: {}", field(summary, "mode")),
        format!("target: {}", field(summary, "target")),
        format!("resolved_kind: {}", field(summary, "resolved_kind")),
        format!("expected_count: {}", field(summary, "expected_count")),
        format!("received_count: {}", received_count),
        format!("terminal_count: {}", terminal_count),
        format!("notice_count: {}", notice_count),
        format!("waited_s: {:.3}", waited_s),
    ];
    if let Some(Value::Array(replies)) = summary.get("replies") {
        for reply in replies {
            let notice = reply.get("notice").and_then(|v| v.as_bool()).unwrap_or(false);
            lines.push(format!(
                "reply: id={} message={} attempt={} agent={} job={} terminal={} notice={} kind={} finished={} reason={}",
                field(reply, "reply_id"),
                field(reply, "message_id"),
                field(reply, "attempt_id"),
                field(reply, "agent_name"),
                field(reply, "job_id"),
                field(reply, "terminal_status"),
                notice,
                field(reply, "notice_kind"),
                field(reply, "finished_at"),
                field(reply, "reason"),
            ));
            if let Some(progress) = reply.get("last_progress_at") {
                if !progress.is_null() {
                    lines.push(format!("reply_last_progress_at: {}", field_value(progress)));
                }
            }
            if let Some(silence) = reply.get("heartbeat_silence_seconds") {
                if !silence.is_null() {
                    lines.push(format!(
                        "reply_heartbeat_silence_seconds: {}",
                        field_value(silence)
                    ));
                }
            }
            lines.push(format!(
                "reply_text: {}",
                display_text(reply.get("reply").unwrap_or(&Value::Null))
            ));
        }
    }
    lines
}

/// Render a watch event batch.
///
/// Mirrors Python `render_watch_batch(batch)`.
pub fn render_watch_batch(batch: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(Value::Array(events)) = batch.get("events") {
        for event in events {
            let target = event_target(event);
            lines.push(format!(
                "event: {} {} {} {} {}",
                field(event, "event_id"),
                field(event, "job_id"),
                target,
                field(event, "type"),
                field(event, "timestamp")
            ));
        }
    }
    if batch.get("terminal").and_then(|v| v.as_bool()).unwrap_or(false) {
        let target = field(batch, "target_name");
        let target = if target.is_empty() {
            field(batch, "agent_name")
        } else {
            target
        };
        lines.push("watch_status: terminal".to_string());
        lines.extend(render_observer_notice("watch", true, "supplementary_snapshot"));
        lines.push(format!("job_id: {}", field(batch, "job_id")));
        lines.push(format!("agent_name: {}", field(batch, "agent_name")));
        lines.push(format!("target_name: {}", target));
        lines.push(format!("status: {}", field(batch, "status")));
        lines.push(format!(
            "reply: {}",
            display_text(batch.get("reply").unwrap_or(&Value::Null))
        ));
    }
    lines
}

/// Render a cancel payload.
///
/// Mirrors Python `render_cancel(payload)`.
pub fn render_cancel(payload: &Value) -> Vec<String> {
    let mut lines = vec!["cancel_status: ok".to_string()];
    lines.extend(render_mapping(payload));
    lines
}

fn job_target(job: &Value) -> String {
    let target = field(job, "target_name");
    if target.is_empty() {
        field(job, "agent_name")
    } else {
        target
    }
}

fn event_target(event: &Value) -> String {
    let target = field(event, "target_name");
    if target.is_empty() {
        field(event, "agent_name")
    } else {
        target
    }
}

fn field_or(value: &Value, key: &str, default: &str) -> String {
    let v = field(value, key);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
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
