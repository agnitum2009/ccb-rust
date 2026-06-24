use std::collections::HashMap;

use serde_json::Value;

use super::handle::ExecutionServiceHandle;
use super::reliability::{adapter_reliability_policy, deadline_at, last_progress_timestamp};

const RUNTIME_STATE_KEYS: &[&str] = &[
    "mode",
    "pane_id",
    "request_anchor",
    "next_seq",
    "anchor_seen",
    "anchor_emitted",
    "no_wrap",
    "bound_turn_id",
    "bound_task_id",
    "last_assistant_uuid",
    "session_path",
    "completion_dir",
    "prompt_sent",
    "prompt_sent_at",
    "ready_wait_started_at",
    "ready_timeout_s",
    "delivery_state",
    "delivery_started_at",
    "delivery_timeout_s",
    "delivery_target_pane_id",
    "delivery_target_session_path",
    "delivery_confirmed_at",
    "delivery_failure_kind",
    "delivery_failed_at",
    "reliability_last_progress_at",
    "reliability_timeout_s",
    "reliability_timeout_deadline_at",
];

pub fn active_runtime_snapshots(handle: &ExecutionServiceHandle) -> Vec<HashMap<String, Value>> {
    let mut snapshots = Vec::new();
    let _now = (handle.clock)();
    let mut active: Vec<_> = handle.active.iter().collect();
    active.sort_by(|a, b| a.0.cmp(b.0));
    for (job_id, submission) in active {
        let adapter = handle.registry.get(&submission.provider_key());
        let policy = adapter.and_then(|a| adapter_reliability_policy(a.as_ref()));
        let mut runtime_state = safe_runtime_state(&submission.runtime_state);
        let delivery_timeout_s = runtime_state
            .get("delivery_timeout_s")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                runtime_state
                    .get("delivery_timeout_s")
                    .and_then(|v| v.as_i64().map(|i| i as f64))
            });
        let delivery_started_at = runtime_state
            .get("delivery_started_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if !delivery_started_at.is_empty() {
            if let Some(timeout_s) = delivery_timeout_s {
                if let Some(deadline) = deadline_at(delivery_started_at, timeout_s) {
                    runtime_state.insert(
                        "delivery_timeout_deadline_at".to_string(),
                        Value::String(deadline),
                    );
                }
            }
        }

        let last_progress_at = last_progress_timestamp(submission);
        let no_terminal_timeout_s = policy.as_ref().map(|p| p.effective_no_terminal_timeout_s());
        let no_terminal_deadline_at = if !last_progress_at.is_empty() {
            no_terminal_timeout_s.and_then(|t| deadline_at(&last_progress_at, t))
        } else {
            None
        };

        let mut snapshot = HashMap::new();
        snapshot.insert("job_id".to_string(), Value::String(job_id.clone()));
        snapshot.insert(
            "agent_name".to_string(),
            Value::String(submission.agent_name.clone()),
        );
        snapshot.insert(
            "provider".to_string(),
            Value::String(submission.provider.clone()),
        );
        snapshot.insert(
            "source_kind".to_string(),
            Value::String(format!("{:?}", submission.source_kind).to_lowercase()),
        );
        snapshot.insert(
            "status".to_string(),
            Value::String(format!("{:?}", submission.status).to_lowercase()),
        );
        snapshot.insert(
            "reason".to_string(),
            Value::String(submission.reason.clone()),
        );
        snapshot.insert(
            "confidence".to_string(),
            Value::String(format!("{:?}", submission.confidence).to_lowercase()),
        );
        snapshot.insert(
            "accepted_at".to_string(),
            Value::String(submission.accepted_at.clone()),
        );
        snapshot.insert(
            "ready_at".to_string(),
            Value::String(submission.ready_at.clone()),
        );
        snapshot.insert(
            "primary_authority".to_string(),
            policy
                .map(|p| Value::String(p.primary_authority.clone()))
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "last_progress_at".to_string(),
            if last_progress_at.is_empty() {
                Value::Null
            } else {
                Value::String(last_progress_at)
            },
        );
        snapshot.insert(
            "no_terminal_timeout_s".to_string(),
            no_terminal_timeout_s
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "no_terminal_deadline_at".to_string(),
            no_terminal_deadline_at
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "runtime_state".to_string(),
            Value::Object(runtime_state.into_iter().collect()),
        );
        snapshots.push(snapshot);
    }
    snapshots
}

fn safe_runtime_state(runtime_state: &HashMap<String, Value>) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    for key in RUNTIME_STATE_KEYS {
        if let Some(value) = runtime_state.get(*key) {
            if let Some(safe) = safe_value(value) {
                result.insert(key.to_string(), safe);
            }
        }
    }
    result
}

fn safe_value(value: &Value) -> Option<Value> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Some(value.clone()),
        _ => None,
    }
}
