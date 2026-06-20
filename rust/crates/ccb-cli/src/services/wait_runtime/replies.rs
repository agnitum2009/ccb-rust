//! Mirrors Python `lib/cli/services/wait_runtime/replies.py`.

use serde_json::Value;
use std::collections::HashMap;

/// Compute the latest reply set from a trace payload.
///
/// Returns `(expected_count, replies, terminal_count, notice_count)`.
pub fn latest_replies(payload: &Value) -> (usize, Vec<Value>, usize, usize) {
    let attempts = payload.get("attempts").and_then(Value::as_array).cloned();
    let replies = payload.get("replies").and_then(Value::as_array).cloned();

    let latest_attempts = latest_attempt_map(attempts.as_deref());
    let replies_by_attempt = latest_reply_map(replies.as_deref());
    let mut replies = materialized_replies(&latest_attempts, &replies_by_attempt);
    replies.sort_by_key(reply_sort_key);

    let notice_count = notice_reply_count(&replies);
    let terminal_count = replies.len() - notice_count;
    (latest_attempts.len(), replies, terminal_count, notice_count)
}

fn latest_attempt_map(attempts: Option<&[Value]>) -> HashMap<(String, String), Value> {
    let mut latest: HashMap<(String, String), Value> = HashMap::new();
    for attempt in attempts.iter().flat_map(|a| a.iter()) {
        let key = attempt_identity(attempt);
        if let Some(current) = latest.get(&key) {
            if attempt_sort_key(attempt) <= attempt_sort_key(current) {
                continue;
            }
        }
        latest.insert(key, attempt.clone());
    }
    latest
}

fn latest_reply_map(replies: Option<&[Value]>) -> HashMap<String, Value> {
    let mut latest: HashMap<String, Value> = HashMap::new();
    for reply in replies.iter().flat_map(|r| r.iter()) {
        let attempt_id = reply
            .get("attempt_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if attempt_id.is_empty() {
            continue;
        }
        if let Some(current) = latest.get(&attempt_id) {
            if reply_sort_key(reply) <= reply_sort_key(current) {
                continue;
            }
        }
        latest.insert(attempt_id, reply.clone());
    }
    latest
}

fn materialized_replies(
    latest_attempts: &HashMap<(String, String), Value>,
    replies_by_attempt: &HashMap<String, Value>,
) -> Vec<Value> {
    let mut replies = Vec::new();
    for attempt in latest_attempts.values() {
        if let Some(reply) = reply_for_attempt(attempt, replies_by_attempt) {
            replies.push(reply);
        }
    }
    replies
}

fn reply_for_attempt(
    attempt: &Value,
    replies_by_attempt: &HashMap<String, Value>,
) -> Option<Value> {
    let attempt_id = attempt
        .get("attempt_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let reply = replies_by_attempt.get(&attempt_id)?;
    let notice = reply
        .get("notice")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Some(serde_json::json!({
        "reply_id": reply.get("reply_id").cloned().unwrap_or(Value::Null),
        "message_id": reply.get("message_id").cloned().unwrap_or(Value::Null),
        "attempt_id": reply.get("attempt_id").cloned().unwrap_or(Value::Null),
        "agent_name": reply.get("agent_name").cloned().unwrap_or(Value::Null),
        "job_id": attempt.get("job_id").cloned().unwrap_or(Value::Null),
        "terminal_status": reply.get("terminal_status").cloned().unwrap_or(Value::Null),
        "notice": notice,
        "notice_kind": reply.get("notice_kind").cloned().unwrap_or(Value::Null),
        "last_progress_at": reply.get("last_progress_at").cloned().unwrap_or(Value::Null),
        "heartbeat_silence_seconds": reply.get("heartbeat_silence_seconds").cloned().unwrap_or(Value::Null),
        "reason": reply.get("reason").cloned().unwrap_or(Value::Null),
        "finished_at": reply.get("finished_at").cloned().unwrap_or(Value::Null),
        "reply": reply.get("reply").and_then(Value::as_str).unwrap_or(""),
    }))
}

fn notice_reply_count(replies: &[Value]) -> usize {
    replies
        .iter()
        .filter(|r| r.get("notice").and_then(Value::as_bool).unwrap_or(false))
        .count()
}

fn attempt_identity(attempt: &Value) -> (String, String) {
    (
        attempt
            .get("message_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        attempt
            .get("agent_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    )
}

fn attempt_sort_key(attempt: &Value) -> (i64, String, String) {
    (
        attempt
            .get("retry_index")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        attempt
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        attempt
            .get("attempt_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    )
}

fn reply_sort_key(reply: &Value) -> (String, String) {
    (
        reply
            .get("finished_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        reply
            .get("reply_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    )
}
