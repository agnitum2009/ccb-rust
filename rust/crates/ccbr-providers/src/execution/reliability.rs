use ccbr_completion::models::{
    CompletionConfidence, CompletionDecision, CompletionItemKind, CompletionStatus,
};

use super::models::{ProviderPollResult, ProviderSubmission};
use serde_json::Value;

use super::common::request_anchor_from_runtime_state;

/// Reliability policy for a provider.
#[derive(Debug, Clone)]
pub struct CompletionReliabilityPolicy {
    pub provider: String,
    pub no_terminal_timeout_s: f64,
    pub primary_authority: String,
    pub backend_type: Option<String>,
    pub timeout_status: CompletionStatus,
    pub timeout_reason: String,
    pub timeout_confidence: CompletionConfidence,
}

impl CompletionReliabilityPolicy {
    pub fn new(
        provider: impl Into<String>,
        no_terminal_timeout_s: f64,
        primary_authority: impl Into<String>,
    ) -> Self {
        let provider = provider.into().trim().to_lowercase();
        assert!(!provider.is_empty(), "provider cannot be empty");
        Self {
            provider,
            no_terminal_timeout_s: no_terminal_timeout_s.max(0.0),
            primary_authority: primary_authority.into(),
            backend_type: Some("pane-backed".to_string()),
            timeout_status: CompletionStatus::Incomplete,
            timeout_reason: "completion_timeout".to_string(),
            timeout_confidence: CompletionConfidence::Degraded,
        }
    }

    pub fn timeout_env_name(&self) -> String {
        format!(
            "CCB_{}_NO_TERMINAL_TIMEOUT_S",
            self.provider.to_uppercase().replace('-', "_")
        )
    }

    pub fn effective_no_terminal_timeout_s(&self) -> f64 {
        let raw = std::env::var(self.timeout_env_name()).unwrap_or_default();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return self.no_terminal_timeout_s;
        }
        trimmed
            .parse::<f64>()
            .map(|v| v.max(0.0))
            .unwrap_or(self.no_terminal_timeout_s)
    }
}

/// Return a reliability policy attached to an adapter, if any.
pub fn adapter_reliability_policy(
    adapter: &dyn super::adapter::ExecutionAdapter,
) -> Option<&CompletionReliabilityPolicy> {
    adapter.reliability_policy()
}

const SEMANTIC_PROGRESS_ITEM_KINDS: &[CompletionItemKind] = &[
    CompletionItemKind::AnchorSeen,
    CompletionItemKind::AssistantChunk,
    CompletionItemKind::AssistantFinal,
    CompletionItemKind::ToolCall,
    CompletionItemKind::ToolResult,
    CompletionItemKind::Result,
    CompletionItemKind::TurnBoundary,
    CompletionItemKind::TurnAborted,
    CompletionItemKind::CancelInfo,
    CompletionItemKind::Error,
    CompletionItemKind::PaneDead,
];

pub fn apply_reliability_progress(
    result: ProviderPollResult,
    previous_submission: &ProviderSubmission,
    now: &str,
) -> ProviderPollResult {
    if !has_reliability_progress(&result, previous_submission) {
        return result;
    }
    let updated = with_last_progress_at(result.submission, now);
    ProviderPollResult::new(updated, result.items, result.decision)
}

fn has_reliability_progress(
    result: &ProviderPollResult,
    previous_submission: &ProviderSubmission,
) -> bool {
    has_semantic_progress_item(result)
        || result.decision.is_some()
        || semantic_progress_marker(&result.submission)
            != semantic_progress_marker(previous_submission)
}

fn has_semantic_progress_item(result: &ProviderPollResult) -> bool {
    result
        .items
        .iter()
        .any(|item| SEMANTIC_PROGRESS_ITEM_KINDS.contains(&item.kind))
}

fn semantic_progress_marker(submission: &ProviderSubmission) -> Vec<String> {
    let state = &submission.runtime_state;
    let get_str = |key: &str| -> String {
        state
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    vec![
        submission.reply.clone(),
        format!("{:?}", submission.status),
        submission.reason.clone(),
        format!("{:?}", submission.confidence),
        format!(
            "{}",
            state
                .get("anchor_seen")
                .or_else(|| state.get("anchor_emitted"))
                .is_some_and(|v| v.as_bool().unwrap_or(false))
        ),
        get_str("bound_turn_id"),
        get_str("bound_task_id"),
        get_str("reply_buffer"),
        get_str("last_agent_message"),
        get_str("last_final_answer"),
        get_str("last_assistant_message"),
        get_str("last_assistant_signature"),
        get_str("session_path"),
    ]
}

fn with_last_progress_at(submission: ProviderSubmission, at: &str) -> ProviderSubmission {
    let mut state = submission.runtime_state.clone();
    if state
        .get("reliability_last_progress_at")
        .and_then(|v| v.as_str())
        == Some(at)
    {
        return submission;
    }
    state.insert(
        "reliability_last_progress_at".to_string(),
        Value::String(at.to_string()),
    );
    ProviderSubmission {
        runtime_state: state,
        ..submission
    }
}

pub fn timeout_poll_result(
    submission: &ProviderSubmission,
    now: &str,
    policy: &CompletionReliabilityPolicy,
) -> Option<ProviderPollResult> {
    let timeout_s = policy.effective_no_terminal_timeout_s();
    if timeout_s <= 0.0 {
        return None;
    }
    let last_progress_at = last_progress_timestamp(submission);
    if !timeout_elapsed(&last_progress_at, now, timeout_s) {
        return None;
    }
    Some(build_timeout_result(
        submission,
        now,
        timeout_s,
        &last_progress_at,
        policy,
    ))
}

pub fn last_progress_timestamp(submission: &ProviderSubmission) -> String {
    let from_state = submission
        .runtime_state
        .get("reliability_last_progress_at")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if let Some(ts) = from_state {
        return ts;
    }
    if !submission.ready_at.trim().is_empty() {
        return submission.ready_at.clone();
    }
    submission.accepted_at.clone()
}

fn timeout_elapsed(started_at: &str, now: &str, timeout_s: f64) -> bool {
    if started_at.trim().is_empty() {
        return false;
    }
    let Ok(start) = ccbr_completion::utils::parse_timestamp(started_at) else {
        return false;
    };
    let Ok(end) = ccbr_completion::utils::parse_timestamp(now) else {
        return false;
    };
    (end - start).num_milliseconds() as f64 / 1000.0 >= timeout_s.max(0.0)
}

fn build_timeout_result(
    submission: &ProviderSubmission,
    now: &str,
    timeout_s: f64,
    last_progress_at: &str,
    policy: &CompletionReliabilityPolicy,
) -> ProviderPollResult {
    let reply = submission.reply.clone();
    let request_anchor =
        request_anchor_from_runtime_state(&submission.runtime_state, &submission.job_id);
    let deadline_at = deadline_at(last_progress_at, timeout_s).unwrap_or_default();
    let mut diagnostics = submission
        .diagnostics
        .clone()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Value::Object(ref mut obj) = diagnostics {
        obj.insert(
            "completion_primary_authority".to_string(),
            Value::String(policy.primary_authority.clone()),
        );
        obj.insert(
            "completion_last_progress_at".to_string(),
            Value::String(last_progress_at.to_string()),
        );
        obj.insert(
            "completion_timeout_s".to_string(),
            Value::Number(serde_json::Number::from_f64(timeout_s).unwrap_or_else(|| 0.into())),
        );
        obj.insert(
            "completion_timeout_deadline_at".to_string(),
            Value::String(deadline_at.clone()),
        );
        obj.insert(
            "completion_reliability_reason".to_string(),
            Value::String(policy.timeout_reason.clone()),
        );
        obj.insert(
            "completion_fallback_source".to_string(),
            Value::String("execution_reliability_monitor".to_string()),
        );
    }
    let mut runtime_state = submission.runtime_state.clone();
    runtime_state.insert(
        "reliability_last_progress_at".to_string(),
        Value::String(last_progress_at.to_string()),
    );
    runtime_state.insert(
        "reliability_timeout_s".to_string(),
        Value::Number(serde_json::Number::from_f64(timeout_s).unwrap_or_else(|| 0.into())),
    );
    runtime_state.insert(
        "reliability_timeout_deadline_at".to_string(),
        Value::String(deadline_at.clone()),
    );
    runtime_state.insert(
        "reliability_terminalized_at".to_string(),
        Value::String(now.to_string()),
    );
    let updated = ProviderSubmission {
        reply,
        status: policy.timeout_status,
        reason: policy.timeout_reason.clone(),
        confidence: policy.timeout_confidence,
        diagnostics: Some(diagnostics.clone()),
        runtime_state,
        ..submission.clone()
    };
    let decision = CompletionDecision {
        terminal: true,
        status: policy.timeout_status,
        reason: Some(policy.timeout_reason.clone()),
        confidence: Some(policy.timeout_confidence),
        reply: updated.reply.clone(),
        anchor_seen: submission
            .runtime_state
            .get("anchor_seen")
            .or_else(|| submission.runtime_state.get("anchor_emitted"))
            .is_some_and(|v| v.as_bool().unwrap_or(false)),
        reply_started: !updated.reply.is_empty(),
        reply_stable: !updated.reply.is_empty(),
        provider_turn_ref: Some(request_anchor),
        source_cursor: None,
        finished_at: Some(now.to_string()),
        diagnostics: diagnostics.as_object().cloned().unwrap_or_default(),
    };
    ProviderPollResult::new(updated, Vec::new(), Some(decision))
}

pub fn deadline_at(started_at: &str, timeout_s: f64) -> Option<String> {
    if started_at.trim().is_empty() {
        return None;
    }
    let start = ccbr_completion::utils::parse_timestamp(started_at).ok()?;
    let deadline = start + chrono::Duration::milliseconds((timeout_s.max(0.0) * 1000.0) as i64);
    Some(
        deadline
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            .replace("+00:00", "Z"),
    )
}
