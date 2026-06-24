//! Mirrors Python `lib/provider_backends/pane_quiet_support/execution.py`.

use std::collections::HashMap;

use ccbr_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionStatus,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::execution::models::{ProviderPollResult, ProviderSubmission};
use crate::pane_quiet_support::protocol::{extract_reply_for_req, pane_contains_req_anchor};
use crate::pane_quiet_support::reader::{PaneContentBackend, PaneSnapshotReader};

const PANE_LINES_DEFAULT: usize = 2000;
const QUIET_SECS: f64 = 4.0;
const MAX_WAIT_SECS: f64 = 300.0;
const MIN_OBSERVED_SECS: f64 = 2.0;
const ANCHOR_WAIT_SECS: f64 = 120.0;
const READY_WAIT_SECS: f64 = 60.0;

/// Backend that can both read pane content and send text to a pane.
///
/// Combines the reader trait with a prompt-sending method.
pub trait PaneQuietBackend: PaneContentBackend + std::fmt::Debug {
    fn send_text_to_pane(&self, pane_id: &str, text: &str);
}

/// Poll a pane-quiet submission.
///
/// Mirrors Python `poll_submission`. The caller supplies the live backend
/// because trait objects cannot be serialized into `runtime_state`.
pub fn poll_submission<B: PaneQuietBackend>(
    submission: &ProviderSubmission,
    backend: &B,
    now: &str,
) -> Option<ProviderPollResult> {
    let mut state = submission.runtime_state.clone();
    let provider = state_str(&state, "provider");
    let provider = if provider.is_empty() {
        submission.provider.clone()
    } else {
        provider
    };

    if let Some(send_error) = state.get("send_error").and_then(Value::as_str) {
        let send_error = send_error.to_string();
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Failed,
            &format!("send_failed:{send_error}"),
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    let pane_id = state_str(&state, "pane_id");
    let req_id = state_str(&state, "req_id");
    if pane_id.is_empty() || req_id.is_empty() {
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Failed,
            "runtime_state_invalid",
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    let reader = PaneSnapshotReader::new(backend, pane_id.clone(), PANE_LINES_DEFAULT);
    let content = reader.snapshot();
    if content.is_empty() {
        let errors = state_i64(&state, "snapshot_errors", 0) + 1;
        state.insert("snapshot_errors".to_string(), Value::from(errors));
    }

    let prompt_sent = state_bool(&state, "prompt_sent", false);
    if !prompt_sent {
        let started_at = state_str(&state, "started_at");
        let started_at = if started_at.is_empty() {
            submission.accepted_at.clone()
        } else {
            started_at
        };
        let ready_wait_secs = seconds_between(&started_at, now);
        state.insert("ready_wait_secs".to_string(), Value::from(ready_wait_secs));

        if requires_ready_before_send(&provider) && !pane_ready_for_input(&content, &provider) {
            if ready_wait_secs >= READY_WAIT_SECS {
                let mut diagnostics = Map::new();
                diagnostics.insert("input_not_ready".to_string(), Value::Bool(true));
                diagnostics.insert("ready_wait_secs".to_string(), Value::from(ready_wait_secs));
                diagnostics.insert(
                    "diagnosis".to_string(),
                    Value::String(format!(
                        "{provider} pane did not reach an input-ready state before prompt delivery."
                    )),
                );
                return Some(terminal(
                    submission,
                    state,
                    now,
                    CompletionStatus::Incomplete,
                    &format!("{provider}_input_not_ready"),
                    "",
                    CompletionConfidence::Degraded,
                    Some(diagnostics),
                ));
            }
            state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
            state.insert(
                "next_seq".to_string(),
                Value::from(state_i64(&state, "next_seq", 1) + 1),
            );
            return Some(progress(submission, state));
        }

        let pending_prompt = state_str(&state, "pending_prompt");
        if pending_prompt.is_empty() {
            let mut diagnostics = Map::new();
            diagnostics.insert("missing_pending_prompt".to_string(), Value::Bool(true));
            return Some(terminal(
                submission,
                state,
                now,
                CompletionStatus::Failed,
                "runtime_state_invalid",
                "",
                CompletionConfidence::Degraded,
                Some(diagnostics),
            ));
        }
        backend.send_text_to_pane(&pane_id, &pending_prompt);
        state.insert("prompt_sent".to_string(), Value::Bool(true));
        state.insert("prompt_sent_at".to_string(), Value::String(now.to_string()));
        state.insert(
            "prompt_deferred_until_ready".to_string(),
            Value::Bool(false),
        );
        state.insert("started_at".to_string(), Value::String(now.to_string()));
        state.insert("last_change_at".to_string(), Value::String(now.to_string()));
        state.insert(
            "last_hash".to_string(),
            if content.is_empty() {
                Value::Null
            } else {
                Value::String(hash_text(&content))
            },
        );
        state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
        state.insert(
            "next_seq".to_string(),
            Value::from(state_i64(&state, "next_seq", 1) + 1),
        );
        return Some(progress(submission, state));
    }

    let current_hash = if content.is_empty() {
        state
            .get("last_hash")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
    } else {
        Some(hash_text(&content))
    };
    let last_hash = state
        .get("last_hash")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let started_at = state_str(&state, "started_at");
    let started_at = if started_at.is_empty() {
        submission.accepted_at.clone()
    } else {
        started_at
    };
    let last_change_at = state_str(&state, "last_change_at");
    let last_change_at = if last_change_at.is_empty() {
        started_at.clone()
    } else {
        last_change_at
    };

    if current_hash.as_ref() != last_hash.as_ref() {
        if let Some(hash) = current_hash {
            state.insert("last_hash".to_string(), Value::String(hash));
            state.insert("last_change_at".to_string(), Value::String(now.to_string()));
        }
    }

    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    state.insert(
        "next_seq".to_string(),
        Value::from(state_i64(&state, "next_seq", 1) + 1),
    );

    let quiet_secs = seconds_between(&last_change_at, now);
    let total_secs = seconds_between(&started_at, now);
    state.insert("quiet_secs".to_string(), Value::from(quiet_secs));
    state.insert("total_secs".to_string(), Value::from(total_secs));

    let (reply, done_seen) = extract_reply_for_req(&content, &req_id);
    state.insert("done_seen".to_string(), Value::Bool(done_seen));
    state.insert("reply_chars".to_string(), Value::from(reply.len() as i64));

    let anchor_present = !content.is_empty() && pane_contains_req_anchor(&content, &req_id);
    state.insert("anchor_present".to_string(), Value::Bool(anchor_present));

    if done_seen && !reply.is_empty() {
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Completed,
            "pane_done_marker",
            &reply,
            CompletionConfidence::Observed,
            None,
        ));
    }

    if done_seen && reply.is_empty() {
        let diagnostics = empty_reply_diagnostics(&provider);
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Incomplete,
            "pane_done_empty_reply",
            "",
            CompletionConfidence::Observed,
            Some(diagnostics),
        ));
    }

    if total_secs >= MAX_WAIT_SECS {
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Failed,
            "pane_quiet_timeout",
            &reply,
            CompletionConfidence::Degraded,
            None,
        ));
    }

    if !reply.is_empty() && total_secs >= MIN_OBSERVED_SECS && quiet_secs >= QUIET_SECS {
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Completed,
            "pane_text_quiet",
            &reply,
            CompletionConfidence::Degraded,
            None,
        ));
    }

    if !anchor_present && total_secs >= ANCHOR_WAIT_SECS {
        return Some(terminal(
            submission,
            state,
            now,
            CompletionStatus::Incomplete,
            &format!("{provider}_input_unresponsive"),
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    Some(progress(submission, state))
}

fn progress(submission: &ProviderSubmission, state: HashMap<String, Value>) -> ProviderPollResult {
    let mut progress = submission.clone();
    progress.runtime_state = state;
    ProviderPollResult::new(progress, Vec::new(), None)
}

#[allow(clippy::too_many_arguments)]
fn terminal(
    submission: &ProviderSubmission,
    state: HashMap<String, Value>,
    now: &str,
    status: CompletionStatus,
    reason: &str,
    reply: &str,
    confidence: CompletionConfidence,
    diagnostics_extra: Option<Map<String, Value>>,
) -> ProviderPollResult {
    let cleaned = reply.to_string();
    let mut progress = submission.clone();
    progress.runtime_state = state.clone();
    progress.status = status;
    progress.reason = reason.to_string();
    progress.reply = cleaned.clone();
    progress.confidence = confidence;

    let event_seq = state_i64(&state, "next_seq", 1) as u64;
    let mut cursor = CompletionCursor::new(submission.source_kind, now.to_string());
    cursor.event_seq = Some(event_seq);

    let mut diagnostics = Map::new();
    diagnostics.insert("mode".to_string(), Value::String("pane_quiet".to_string()));
    diagnostics.insert(
        "quiet_secs".to_string(),
        Value::from(
            state
                .get("quiet_secs")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        ),
    );
    diagnostics.insert(
        "total_secs".to_string(),
        Value::from(
            state
                .get("total_secs")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        ),
    );
    diagnostics.insert(
        "done_seen".to_string(),
        Value::Bool(
            state
                .get("done_seen")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    diagnostics.insert(
        "anchor_present".to_string(),
        Value::Bool(
            state
                .get("anchor_present")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    diagnostics.insert(
        "snapshot_errors".to_string(),
        Value::from(state_i64(&state, "snapshot_errors", 0)),
    );
    diagnostics.insert(
        "reply_chars".to_string(),
        Value::from(state_i64(&state, "reply_chars", 0)),
    );
    if let Some(extra) = diagnostics_extra {
        for (k, v) in extra {
            diagnostics.insert(k, v);
        }
    }

    let anchor_seen = state
        .get("anchor_present")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || state
            .get("done_seen")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || !cleaned.is_empty();

    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason.to_string()),
        confidence: Some(confidence),
        reply: cleaned.clone(),
        anchor_seen,
        reply_started: !cleaned.is_empty(),
        reply_stable: !cleaned.is_empty(),
        provider_turn_ref: state
            .get("req_id")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    ProviderPollResult::new(progress, Vec::new(), Some(decision))
}

fn empty_reply_diagnostics(provider: &str) -> Map<String, Value> {
    let diagnosis = format!(
        "{provider} pane showed the requested done marker without assistant \
         reply text; inspect the pane transcript and provider auth/API output."
    );
    let mut map = Map::new();
    map.insert("empty_reply".to_string(), Value::Bool(true));
    map.insert(
        "error_type".to_string(),
        Value::String("empty_provider_reply".to_string()),
    );
    map.insert("message".to_string(), Value::String(diagnosis.clone()));
    map.insert("diagnosis".to_string(), Value::String(diagnosis));
    map
}

fn requires_ready_before_send(provider: &str) -> bool {
    provider.trim().eq_ignore_ascii_case("kimi")
}

fn pane_ready_for_input(content: &str, provider: &str) -> bool {
    if !provider.trim().eq_ignore_ascii_case("kimi") {
        return true;
    }
    let text = content;
    let legacy_ready = text.contains("── input") && text.contains("agent (");
    let k27_ready = text.contains("│ >") && text.contains("K2.7 Code") && text.contains("context:");
    legacy_ready || k27_ready
}

fn hash_text(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn seconds_between(start: &str, end: &str) -> f64 {
    let start_dt = parse_now(start);
    let end_dt = parse_now(end);
    match (start_dt, end_dt) {
        (Some(s), Some(e)) => ((e - s).num_milliseconds() as f64 / 1000.0).max(0.0),
        _ => 0.0,
    }
}

fn parse_now(now: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if now.is_empty() {
        return None;
    }
    let normalized = now.replace("Z", "+00:00");
    chrono::DateTime::parse_from_rfc3339(&normalized)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn state_str(state: &HashMap<String, Value>, key: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn state_i64(state: &HashMap<String, Value>, key: &str, default: i64) -> i64 {
    state
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(default)
}

fn state_bool(state: &HashMap<String, Value>, key: &str, default: bool) -> bool {
    state.get(key).and_then(Value::as_bool).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_completion::models::CompletionSourceKind;
    use std::cell::RefCell;

    #[derive(Debug)]
    struct TestBackend {
        text: RefCell<String>,
        sent_texts: RefCell<Vec<String>>,
    }

    impl PaneContentBackend for TestBackend {
        fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Option<String> {
            Some(self.text.borrow().clone())
        }
    }

    impl PaneQuietBackend for TestBackend {
        fn send_text_to_pane(&self, _pane_id: &str, text: &str) {
            self.sent_texts.borrow_mut().push(text.to_string());
        }
    }

    fn submission(_text: &str, provider: &str, prompt_sent: bool) -> ProviderSubmission {
        let req_id = "job_native123".to_string();
        let mut runtime_state = HashMap::new();
        runtime_state.insert("mode".to_string(), Value::String("pane_quiet".to_string()));
        runtime_state.insert("provider".to_string(), Value::String(provider.to_string()));
        runtime_state.insert("pane_id".to_string(), Value::String("%9".to_string()));
        runtime_state.insert("req_id".to_string(), Value::String(req_id.clone()));
        runtime_state.insert(
            "started_at".to_string(),
            Value::String("2026-06-13T00:00:00Z".to_string()),
        );
        runtime_state.insert(
            "last_change_at".to_string(),
            Value::String("2026-06-13T00:00:00Z".to_string()),
        );
        runtime_state.insert("prompt_sent".to_string(), Value::Bool(prompt_sent));
        runtime_state.insert(
            "pending_prompt".to_string(),
            Value::String("pending prompt".to_string()),
        );
        runtime_state.insert("next_seq".to_string(), Value::from(1));

        ProviderSubmission {
            job_id: req_id.clone(),
            agent_name: format!("{provider}_agent"),
            provider: provider.to_string(),
            accepted_at: "2026-06-13T00:00:00Z".to_string(),
            ready_at: "2026-06-13T00:00:00Z".to_string(),
            source_kind: CompletionSourceKind::TerminalText,
            reply: String::new(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".to_string(),
            confidence: CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state,
        }
    }

    #[test]
    fn test_pane_quiet_poll_marks_done_marker_with_reply_completed() {
        let text = "CCBR_REQ_ID: job_native123\nIMPORTANT: when you finish answering\nCCBR_DONE: job_native123\nfinal answer\nCCBR_DONE: job_native123\n";
        let backend = TestBackend {
            text: RefCell::new(text.to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission(text, "kimi", true),
            &backend,
            "2026-06-13T00:00:03Z",
        );
        assert!(result.is_some());
        let decision = result
            .unwrap()
            .decision
            .expect("decision should be present");
        assert_eq!(decision.status, CompletionStatus::Completed);
        assert_eq!(decision.reason, Some("pane_done_marker".to_string()));
        assert_eq!(decision.reply, "final answer");
    }

    #[test]
    fn test_pane_quiet_poll_defers_kimi_prompt_until_input_ready() {
        let backend = TestBackend {
            text: RefCell::new("Kimi is booting\n".to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission("Kimi is booting\n", "kimi", false),
            &backend,
            "2026-06-13T00:00:03Z",
        );
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.decision.is_none());
        assert!(backend.sent_texts.borrow().is_empty());
        assert_eq!(
            result
                .submission
                .runtime_state
                .get("prompt_sent")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn test_pane_quiet_poll_sends_deferred_kimi_prompt_when_input_ready() {
        let text = "Welcome to Kimi Code CLI!\n── input ─────────\nagent (kimi-for-coding ○)\n";
        let backend = TestBackend {
            text: RefCell::new(text.to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission(text, "kimi", false),
            &backend,
            "2026-06-13T00:00:03Z",
        );
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.decision.is_none());
        assert_eq!(backend.sent_texts.borrow().as_slice(), &["pending prompt"]);
        assert_eq!(
            result
                .submission
                .runtime_state
                .get("prompt_sent")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            result
                .submission
                .runtime_state
                .get("prompt_deferred_until_ready")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn test_pane_quiet_poll_sends_deferred_kimi_prompt_with_k27_input_box() {
        let text = "✦ K2.7 Code is ready higher end-to-end coding task success rates\n╭────────────────────────────────────────────────────────╮\n│ >                                                      │\n╰────────────────────────────────────────────────────────╯\nyolo  K2.7 Code thinking  /home/agnitum/o13  context: 0.0% (0/262.1k)\n";
        let backend = TestBackend {
            text: RefCell::new(text.to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission(text, "kimi", false),
            &backend,
            "2026-06-13T00:00:03Z",
        );
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.decision.is_none());
        assert_eq!(backend.sent_texts.borrow().as_slice(), &["pending prompt"]);
        assert_eq!(
            result
                .submission
                .runtime_state
                .get("prompt_sent")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn test_pane_quiet_poll_reports_kimi_input_not_ready_timeout() {
        let backend = TestBackend {
            text: RefCell::new("Kimi is booting\n".to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission("Kimi is booting\n", "kimi", false),
            &backend,
            "2026-06-13T00:02:00Z",
        );
        assert!(result.is_some());
        let decision = result
            .unwrap()
            .decision
            .expect("decision should be present");
        assert_eq!(decision.status, CompletionStatus::Incomplete);
        assert_eq!(decision.reason, Some("kimi_input_not_ready".to_string()));
        assert_eq!(
            decision
                .diagnostics
                .get("input_not_ready")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn test_pane_quiet_poll_marks_done_marker_with_empty_reply_incomplete() {
        let text = "CCBR_REQ_ID: job_native123\nIMPORTANT: when you finish answering\nCCBR_DONE: job_native123\nCCBR_DONE: job_native123\n";
        let backend = TestBackend {
            text: RefCell::new(text.to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission(text, "deepseek", true),
            &backend,
            "2026-06-13T00:00:03Z",
        );
        assert!(result.is_some());
        let decision = result
            .unwrap()
            .decision
            .expect("decision should be present");
        assert_eq!(decision.status, CompletionStatus::Incomplete);
        assert_eq!(decision.reason, Some("pane_done_empty_reply".to_string()));
        assert_eq!(
            decision
                .diagnostics
                .get("empty_reply")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            decision
                .diagnostics
                .get("error_type")
                .and_then(Value::as_str),
            Some("empty_provider_reply")
        );
        assert!(decision
            .diagnostics
            .get("diagnosis")
            .and_then(Value::as_str)
            .unwrap()
            .contains("deepseek pane showed"));
    }

    #[test]
    fn test_pane_quiet_poll_reports_input_unresponsive_when_anchor_never_appears() {
        let backend = TestBackend {
            text: RefCell::new("provider prompt\n".to_string()),
            sent_texts: RefCell::new(Vec::new()),
        };
        let result = poll_submission(
            &submission("provider prompt\n", "kimi", true),
            &backend,
            "2026-06-13T00:03:00Z",
        );
        assert!(result.is_some());
        let decision = result
            .unwrap()
            .decision
            .expect("decision should be present");
        assert_eq!(decision.status, CompletionStatus::Incomplete);
        assert_eq!(decision.reason, Some("kimi_input_unresponsive".to_string()));
    }
}
