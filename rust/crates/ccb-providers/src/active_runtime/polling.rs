//! Mirrors Python `lib/provider_execution/active_runtime/polling_runtime/`.

use std::collections::HashMap;

use ccb_completion::models::{
    CompletionConfidence, CompletionDecision, CompletionItemKind, CompletionStatus,
};
use serde_json::Value;

use crate::execution::{build_item, ProviderPollResult, ProviderSubmission};

use super::models::PreparedActivePoll;

/// Outcome of preparing an active poll.
///
/// Mirrors the Python union `ProviderPollResult | PreparedActivePoll | None`.
#[derive(Debug, Clone)]
pub enum ActivePollOutcome {
    Result(Box<ProviderPollResult>),
    Prepared(PreparedActivePoll<Value>),
}

/// Prepare an active poll, optionally checking that the target pane is alive.
///
/// Mirrors Python `prepare_active_poll`.
pub fn prepare_active_poll(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ActivePollOutcome> {
    _prepare_active_poll(submission, now, true)
}

/// Prepare an active poll without verifying pane liveness.
///
/// Mirrors Python `prepare_active_poll_without_liveness`.
pub fn prepare_active_poll_without_liveness(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ActivePollOutcome> {
    _prepare_active_poll(submission, now, false)
}

fn _prepare_active_poll(
    submission: &ProviderSubmission,
    now: &str,
    check_pane_alive: bool,
) -> Option<ActivePollOutcome> {
    if let Some(result) = runtime_mode_error(submission, now) {
        return Some(ActivePollOutcome::Result(Box::new(result)));
    }

    let state = &submission.runtime_state;
    let reader = state.get("reader")?;
    let backend = state.get("backend")?;
    let pane_id = state
        .get("pane_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if pane_id.is_empty() {
        return Some(ActivePollOutcome::Result(Box::new(runtime_error_result(
            submission,
            now,
            "runtime_state_corrupt",
            "",
        ))));
    }

    if check_pane_alive {
        if let Some(result) = ensure_active_pane_alive(submission, backend, &pane_id, now) {
            return Some(ActivePollOutcome::Result(Box::new(result)));
        }
    }

    Some(ActivePollOutcome::Prepared(PreparedActivePoll {
        reader: reader.clone(),
        backend: backend.clone(),
        pane_id,
    }))
}

/// Return a runtime-error poll result when the submission mode is passive or
/// error. Mirrors Python `runtime_mode_error`.
pub fn runtime_mode_error(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    let mode = submission
        .runtime_state
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("passive");
    match mode {
        "passive" => Some(runtime_error_result(
            submission,
            now,
            submission
                .runtime_state
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("runtime_unavailable"),
            submission
                .runtime_state
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )),
        "error" => Some(runtime_error_result(
            submission,
            now,
            submission
                .runtime_state
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("transport_error"),
            submission
                .runtime_state
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )),
        _ => None,
    }
}

/// Verify the runtime target pane is alive and emit a `pane_dead` result if not.
///
/// Mirrors Python `ensure_active_pane_alive`.
pub fn ensure_active_pane_alive(
    submission: &ProviderSubmission,
    backend: &Value,
    pane_id: &str,
    now: &str,
) -> Option<ProviderPollResult> {
    if is_runtime_target_alive(backend, pane_id) {
        return None;
    }
    Some(pane_dead_result(submission, now))
}

/// Default runtime-target liveness check for mock/value backends.
///
/// Mirrors the duck-typed Python `is_runtime_target_alive`: if `backend` is an
/// object with an `alive_panes` array, returns true when it contains `pane_id`.
pub fn is_runtime_target_alive(backend: &Value, pane_id: &str) -> bool {
    backend
        .get("alive_panes")
        .and_then(Value::as_array)
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(pane_id)))
}

/// Build a terminal poll result indicating the runtime pane died.
///
/// Mirrors Python `pane_dead_result`.
pub fn pane_dead_result(submission: &ProviderSubmission, now: &str) -> ProviderPollResult {
    let reason = "pane_dead";
    let seq = next_seq(submission);
    let item = build_item(
        submission,
        CompletionItemKind::PaneDead,
        now,
        seq,
        HashMap::from_iter([("reason".to_string(), Value::String(reason.to_string()))]),
    );
    let next = item.cursor.event_seq.unwrap_or(seq) + 1;

    let mut updated = submission.clone();
    updated
        .runtime_state
        .insert("mode".to_string(), Value::String("passive".to_string()));
    updated
        .runtime_state
        .insert("next_seq".to_string(), Value::Number(next.into()));

    let cursor = item.cursor.clone();
    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some(reason.to_string()),
        confidence: Some(CompletionConfidence::Degraded),
        reply: String::new(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: None,
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics: serde_json::Map::from_iter([(
            "reason".to_string(),
            Value::String(reason.to_string()),
        )]),
    };
    ProviderPollResult::new(updated, vec![item], Some(decision))
}

/// Build a terminal poll result for a runtime-level error.
///
/// Mirrors Python `runtime_error_result`.
pub fn runtime_error_result(
    submission: &ProviderSubmission,
    now: &str,
    reason: &str,
    error: &str,
) -> ProviderPollResult {
    let seq = next_seq(submission);
    let mut payload = HashMap::new();
    payload.insert("reason".to_string(), Value::String(reason.to_string()));
    if !error.is_empty() {
        payload.insert("error".to_string(), Value::String(error.to_string()));
    }
    let item = build_item(submission, CompletionItemKind::Error, now, seq, payload);
    let next = item.cursor.event_seq.unwrap_or(seq) + 1;

    let mut updated = submission.clone();
    updated
        .runtime_state
        .insert("mode".to_string(), Value::String("passive".to_string()));
    updated
        .runtime_state
        .insert("next_seq".to_string(), Value::Number(next.into()));

    let cursor = item.cursor.clone();
    let mut diagnostics = serde_json::Map::new();
    diagnostics.insert("reason".to_string(), Value::String(reason.to_string()));
    if !error.is_empty() {
        diagnostics.insert("error".to_string(), Value::String(error.to_string()));
        diagnostics.insert(
            "error_message".to_string(),
            Value::String(error.to_string()),
        );
    }
    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some(reason.to_string()),
        confidence: Some(CompletionConfidence::Degraded),
        reply: String::new(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: None,
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    ProviderPollResult::new(updated, vec![item], Some(decision))
}

fn next_seq(submission: &ProviderSubmission) -> u64 {
    submission
        .runtime_state
        .get("next_seq")
        .and_then(Value::as_u64)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_completion::models::{CompletionSourceKind, CompletionStatus};

    fn submission(mode: &str) -> ProviderSubmission {
        let mut runtime_state = HashMap::new();
        runtime_state.insert("mode".to_string(), Value::String(mode.to_string()));
        ProviderSubmission {
            job_id: "job_1".to_string(),
            agent_name: "agent1".to_string(),
            provider: "codex".to_string(),
            accepted_at: "2026-04-06T00:00:00Z".to_string(),
            ready_at: "2026-04-06T00:00:00Z".to_string(),
            source_kind: CompletionSourceKind::SessionEventLog,
            reply: String::new(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".to_string(),
            confidence: CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state,
        }
    }

    #[test]
    fn test_prepare_active_poll_returns_runtime_error_for_passive_mode() {
        let mut sub = submission("passive");
        sub.runtime_state.insert(
            "reason".to_string(),
            Value::String("runtime_unavailable".to_string()),
        );
        sub.runtime_state.insert(
            "error".to_string(),
            Value::String("missing_reader".to_string()),
        );

        let outcome = prepare_active_poll(&sub, "2026-04-06T00:00:01Z").expect("expected result");
        let ActivePollOutcome::Result(result) = outcome else {
            panic!("expected poll result, got prepared");
        };
        assert_eq!(result.items[0].kind, CompletionItemKind::Error);
        let decision = result.decision.expect("expected decision");
        assert_eq!(decision.status, CompletionStatus::Failed);
        assert_eq!(decision.reason.as_deref(), Some("runtime_unavailable"));
        assert_eq!(
            decision.diagnostics.get("error").unwrap().as_str().unwrap(),
            "missing_reader"
        );
    }

    #[test]
    fn test_ensure_active_pane_alive_marks_dead_pane() {
        let mut sub = submission("active");
        sub.runtime_state
            .insert("next_seq".to_string(), Value::Number(4.into()));

        let backend = serde_json::json!({"alive_panes": ["%1"]});
        let result = ensure_active_pane_alive(&sub, &backend, "%7", "2026-04-06T00:00:01Z")
            .expect("expected pane dead result");

        assert_eq!(result.items[0].kind, CompletionItemKind::PaneDead);
        assert_eq!(result.items[0].cursor.event_seq, Some(4));
        let decision = result.decision.expect("expected decision");
        assert_eq!(decision.status, CompletionStatus::Failed);
        assert_eq!(decision.reason.as_deref(), Some("pane_dead"));
    }
}
