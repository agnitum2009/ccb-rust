use std::collections::HashMap;

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionFamily,
    CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord, SelectorFamily,
};
use ccb_provider_core::contracts::ProviderBackend;
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use serde_json::Value;

use crate::execution::{
    build_item, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};

pub const PROVIDER_NAME_FAKE: &str = "fake";
pub const PROVIDER_NAME_FAKE_CODEX: &str = "fake-codex";
pub const PROVIDER_NAME_FAKE_CLAUDE: &str = "fake-claude";
pub const PROVIDER_NAME_FAKE_GEMINI: &str = "fake-gemini";
pub const PROVIDER_NAME_FAKE_LEGACY: &str = "fake-legacy";

/// Provider names used as fake/test-double backends.
pub const TEST_DOUBLE_PROVIDER_NAMES: &[&str] = &[
    PROVIDER_NAME_FAKE,
    PROVIDER_NAME_FAKE_CODEX,
    PROVIDER_NAME_FAKE_CLAUDE,
    PROVIDER_NAME_FAKE_GEMINI,
    PROVIDER_NAME_FAKE_LEGACY,
];

const DEFAULT_LATENCY_MS: u64 = 200;
const DEFAULT_POLLS_UNTIL_COMPLETE: u64 = 1;

/// Script mode controlling the default event sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeScriptMode {
    StructuredResult,
    ProtocolTurn,
    SessionBoundary,
    AnchoredSessionStability,
    LegacyText,
}

impl FakeScriptMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::StructuredResult => "structured_result",
            Self::ProtocolTurn => "protocol_turn",
            Self::SessionBoundary => "session_boundary",
            Self::AnchoredSessionStability => "anchored_session_stability",
            Self::LegacyText => "legacy_text",
        }
    }
}

/// Runtime configuration parsed from provider options.
#[derive(Debug, Clone)]
struct FakeConfig {
    status: CompletionStatus,
    reason: String,
    confidence: CompletionConfidence,
    reply: Option<String>,
    polls_until_complete: u64,
    latency_ms: u64,
}

impl Default for FakeConfig {
    fn default() -> Self {
        Self {
            status: CompletionStatus::Completed,
            reason: "result_message".to_string(),
            confidence: CompletionConfidence::Exact,
            reply: None,
            polls_until_complete: DEFAULT_POLLS_UNTIL_COMPLETE,
            latency_ms: DEFAULT_LATENCY_MS,
        }
    }
}

/// Source kind and script mode for a fake provider variant.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct FakeVariant {
    source_kind: CompletionSourceKind,
    script_mode: FakeScriptMode,
    completion_family: CompletionFamily,
    selector_family: SelectorFamily,
    supports_exact_completion: bool,
    supports_observed_completion: bool,
    supports_anchor_binding: bool,
    supports_reply_stability: bool,
    supports_terminal_reason: bool,
}

impl FakeVariant {
    fn for_provider(provider: &str) -> Self {
        match provider {
            PROVIDER_NAME_FAKE_CODEX => Self {
                source_kind: CompletionSourceKind::ProtocolEventStream,
                script_mode: FakeScriptMode::ProtocolTurn,
                completion_family: CompletionFamily::ProtocolTurn,
                selector_family: SelectorFamily::FinalMessage,
                supports_exact_completion: true,
                supports_observed_completion: false,
                supports_anchor_binding: true,
                supports_reply_stability: false,
                supports_terminal_reason: true,
            },
            PROVIDER_NAME_FAKE_CLAUDE => Self {
                source_kind: CompletionSourceKind::SessionEventLog,
                script_mode: FakeScriptMode::SessionBoundary,
                completion_family: CompletionFamily::SessionBoundary,
                selector_family: SelectorFamily::FinalMessage,
                supports_exact_completion: false,
                supports_observed_completion: true,
                supports_anchor_binding: true,
                supports_reply_stability: false,
                supports_terminal_reason: true,
            },
            PROVIDER_NAME_FAKE_GEMINI => Self {
                source_kind: CompletionSourceKind::SessionSnapshot,
                script_mode: FakeScriptMode::AnchoredSessionStability,
                completion_family: CompletionFamily::AnchoredSessionStability,
                selector_family: SelectorFamily::SessionReply,
                supports_exact_completion: false,
                supports_observed_completion: true,
                supports_anchor_binding: true,
                supports_reply_stability: true,
                supports_terminal_reason: true,
            },
            PROVIDER_NAME_FAKE_LEGACY => Self {
                source_kind: CompletionSourceKind::TerminalText,
                script_mode: FakeScriptMode::LegacyText,
                completion_family: CompletionFamily::TerminalTextQuiet,
                selector_family: SelectorFamily::FinalMessage,
                supports_exact_completion: false,
                supports_observed_completion: false,
                supports_anchor_binding: false,
                supports_reply_stability: false,
                supports_terminal_reason: false,
            },
            _ => Self {
                source_kind: CompletionSourceKind::StructuredResultStream,
                script_mode: FakeScriptMode::StructuredResult,
                completion_family: CompletionFamily::StructuredResult,
                selector_family: SelectorFamily::StructuredResult,
                supports_exact_completion: true,
                supports_observed_completion: false,
                supports_anchor_binding: true,
                supports_reply_stability: false,
                supports_terminal_reason: true,
            },
        }
    }
}

/// Generic fake execution adapter.
///
/// Mirrors Python `provider_execution.fake.FakeProviderAdapter`. The adapter can
/// be configured through `provider_options`:
///
/// - `status`: `"completed"` (default), `"cancelled"`, `"failed"`, `"incomplete"`
/// - `reason`: terminal reason string
/// - `reply`: fixed reply text (default: `FAKE[{agent_name}] {body}`)
/// - `polls_until_complete`: number of polls before terminal decision (default: 1)
/// - `confidence`: `"exact"` (default), `"observed"`, `"degraded"`
/// - `latency_ms`: simulated latency in milliseconds (default: 200)
#[derive(Debug, Clone)]
pub struct FakeExecutionAdapter {
    provider: String,
    variant: FakeVariant,
}

impl FakeExecutionAdapter {
    /// Create a fake adapter for the named provider variant.
    pub fn new(provider: impl Into<String>) -> Self {
        let provider = provider.into().trim().to_lowercase();
        let variant = FakeVariant::for_provider(&provider);
        Self { provider, variant }
    }

    /// Return the configured provider name.
    pub fn provider_name(&self) -> &str {
        &self.provider
    }

    /// Return the source kind for this variant.
    pub fn source_kind(&self) -> CompletionSourceKind {
        self.variant.source_kind
    }

    /// Return the script mode for this variant.
    pub fn script_mode(&self) -> &str {
        self.variant.script_mode.as_str()
    }
}

impl ExecutionAdapter for FakeExecutionAdapter {
    fn provider(&self) -> &str {
        &self.provider
    }

    fn start(
        &self,
        job: &JobRecord,
        _context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        let config = parse_config(&job.provider_options);
        let reply = config
            .reply
            .clone()
            .unwrap_or_else(|| format!("FAKE[{}] {}", job.agent_name, job.request.body));
        let ready_at = add_ms(now, config.latency_ms);

        let diagnostics = serde_json::json!({
            "provider": self.provider,
            "task_id": job.request.message_type.as_deref().unwrap_or(""),
        });

        let mut runtime_state = HashMap::new();
        runtime_state.insert("poll_count".to_string(), Value::Number(0.into()));
        runtime_state.insert(
            "polls_until_complete".to_string(),
            Value::Number(config.polls_until_complete.into()),
        );
        runtime_state.insert("reply".to_string(), Value::String(reply));
        runtime_state.insert(
            "status".to_string(),
            Value::String(status_to_str(config.status).to_string()),
        );
        runtime_state.insert("reason".to_string(), Value::String(config.reason.clone()));
        runtime_state.insert(
            "confidence".to_string(),
            Value::String(confidence_to_str(config.confidence).to_string()),
        );
        runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));

        ProviderSubmission {
            job_id: job.job_id.clone(),
            agent_name: job.agent_name.clone(),
            provider: self.provider.clone(),
            accepted_at: now.to_string(),
            ready_at,
            source_kind: self.variant.source_kind,
            reply: config
                .reply
                .unwrap_or_else(|| format!("FAKE[{}] {}", job.agent_name, job.request.body)),
            status: CompletionStatus::Incomplete,
            reason: config.reason,
            confidence: config.confidence,
            diagnostics: Some(diagnostics),
            runtime_state,
        }
    }

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        if submission.is_terminal() {
            return None;
        }

        let mut state = submission.runtime_state.clone();
        let poll_count = state
            .get("poll_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        state.insert("poll_count".to_string(), Value::Number(poll_count.into()));

        let polls_until_complete = state
            .get("polls_until_complete")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_POLLS_UNTIL_COMPLETE);
        let reply = state
            .get("reply")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let terminal_status = str_to_status(
            state
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed"),
        );
        let terminal_reason = state
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("result_message")
            .to_string();
        let terminal_confidence = str_to_confidence(
            state
                .get("confidence")
                .and_then(Value::as_str)
                .unwrap_or("exact"),
        );
        let next_seq = state.get("next_seq").and_then(Value::as_u64).unwrap_or(1);

        let mut items = Vec::new();

        if poll_count < polls_until_complete {
            // Emit a progress item appropriate for the variant.
            let progress_kind = progress_item_kind(self.variant.script_mode);
            let mut payload = HashMap::new();
            payload.insert("text".to_string(), Value::String(reply.clone()));
            if matches!(progress_kind, CompletionItemKind::SessionSnapshot) {
                payload.insert("reply".to_string(), Value::String(reply.clone()));
                payload.insert("content".to_string(), Value::String(reply.clone()));
            }
            items.push(build_item(
                submission,
                progress_kind,
                now,
                next_seq,
                payload,
            ));
            state.insert("next_seq".to_string(), Value::Number((next_seq + 1).into()));

            let updated = ProviderSubmission {
                runtime_state: state,
                ..submission.clone()
            };
            return Some(ProviderPollResult::new(updated, items, None));
        }

        // Terminal poll: emit the terminal item and decision.
        let terminal_kind = terminal_item_kind(terminal_status, self.variant.script_mode);
        let mut payload = HashMap::new();
        payload.insert("reply".to_string(), Value::String(reply.clone()));
        payload.insert("reason".to_string(), Value::String(terminal_reason.clone()));
        payload.insert(
            "status".to_string(),
            Value::String(status_to_str(terminal_status).to_string()),
        );
        match terminal_kind {
            CompletionItemKind::AssistantFinal => {
                payload.insert("text".to_string(), Value::String(reply.clone()));
                payload.insert("done_marker".to_string(), Value::Bool(true));
            }
            CompletionItemKind::SessionSnapshot => {
                payload.insert("text".to_string(), Value::String(reply.clone()));
                payload.insert("content".to_string(), Value::String(reply.clone()));
                payload.insert("done_marker_seen".to_string(), Value::Bool(true));
            }
            CompletionItemKind::TurnBoundary | CompletionItemKind::Result => {
                payload.insert(
                    "last_agent_message".to_string(),
                    Value::String(reply.clone()),
                );
            }
            CompletionItemKind::TurnAborted => {
                payload.insert(
                    "status".to_string(),
                    Value::String("incomplete".to_string()),
                );
            }
            _ => {}
        }

        let item = build_item(submission, terminal_kind, now, next_seq, payload);
        state.insert("next_seq".to_string(), Value::Number((next_seq + 1).into()));
        items.push(item.clone());

        let cursor = CompletionCursor {
            source_kind: submission.source_kind,
            event_seq: Some(next_seq),
            updated_at: Some(now.to_string()),
            ..Default::default()
        };
        let decision = CompletionDecision {
            terminal: true,
            status: terminal_status,
            reason: Some(terminal_reason),
            confidence: Some(terminal_confidence),
            reply: reply.clone(),
            anchor_seen: item.kind == CompletionItemKind::SessionSnapshot
                || self.variant.script_mode == FakeScriptMode::ProtocolTurn
                || self.variant.script_mode == FakeScriptMode::StructuredResult
                || self.variant.script_mode == FakeScriptMode::AnchoredSessionStability,
            reply_started: true,
            reply_stable: true,
            provider_turn_ref: Some(submission.job_id.clone()),
            source_cursor: Some(cursor),
            finished_at: Some(now.to_string()),
            diagnostics: item.payload.clone(),
        };

        let updated = ProviderSubmission {
            reply,
            status: terminal_status,
            runtime_state: state,
            ..submission.clone()
        };
        Some(ProviderPollResult::new(updated, items, Some(decision)))
    }

    fn export_runtime_state(
        &self,
        submission: &ProviderSubmission,
    ) -> Option<HashMap<String, Value>> {
        Some(submission.runtime_state.clone())
    }

    fn resume(
        &self,
        _job: &JobRecord,
        submission: &ProviderSubmission,
        _context: Option<&ProviderRuntimeContext>,
        _persisted_state: &crate::execution::PersistedExecutionState,
        _now: &str,
    ) -> Option<ProviderSubmission> {
        Some(submission.clone())
    }
}

/// Build all fake execution adapters.
pub fn execution_adapters() -> Vec<FakeExecutionAdapter> {
    TEST_DOUBLE_PROVIDER_NAMES
        .iter()
        .map(|name| FakeExecutionAdapter::new(*name))
        .collect()
}

/// Build the provider manifest for a fake provider variant.
pub fn manifest(provider: &str) -> ProviderManifest {
    let provider = provider.trim().to_lowercase();
    let _variant = FakeVariant::for_provider(&provider);
    let mut profiles = HashMap::new();
    profiles.insert(
        RuntimeMode::PaneBacked,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "pane-backed".to_string(),
            poll_interval_ms: 50,
            timeout_ms: 30_000,
            ..Default::default()
        },
    );
    if provider == PROVIDER_NAME_FAKE {
        profiles.insert(
            RuntimeMode::Headless,
            CompletionManifest {
                provider: provider.clone(),
                runtime_mode: "headless".to_string(),
                poll_interval_ms: 50,
                timeout_ms: 30_000,
                ..Default::default()
            },
        );
    }
    ProviderManifest::new(
        provider, true,  // supports_resume
        true,  // supports_permission_auto
        true,  // supports_stream_watch
        false, // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

/// Build a fake provider backend with manifest only (no session/launcher).
pub fn backend(provider: &str) -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(provider),
        execution_adapter: None,
        session_binding: None,
        runtime_launcher: None,
    }
}

/// Build all fake provider backends.
pub fn backends() -> Vec<ProviderBackend> {
    TEST_DOUBLE_PROVIDER_NAMES
        .iter()
        .map(|name| backend(name))
        .collect()
}

// ---------------------------------------------------------------------------
// Configuration parsing
// ---------------------------------------------------------------------------

fn parse_config(options: &serde_json::Map<String, Value>) -> FakeConfig {
    let mut config = FakeConfig::default();

    if let Some(directive) = options
        .get("directive")
        .and_then(Value::as_str)
        .or_else(|| options.get("task_id").and_then(Value::as_str))
    {
        apply_directive(&mut config, directive);
    }

    if let Some(status) = options.get("status").and_then(Value::as_str) {
        config.status = str_to_status(status);
    }
    if let Some(reason) = options.get("reason").and_then(Value::as_str) {
        config.reason = reason.to_string();
    }
    if let Some(reply) = options.get("reply").and_then(Value::as_str) {
        config.reply = Some(reply.to_string());
    }
    if let Some(polls) = options.get("polls_until_complete").and_then(Value::as_u64) {
        config.polls_until_complete = polls.max(1);
    }
    if let Some(latency) = options.get("latency_ms").and_then(Value::as_u64) {
        config.latency_ms = latency;
    }
    if let Some(confidence) = options.get("confidence").and_then(Value::as_str) {
        config.confidence = str_to_confidence(confidence);
    }

    config
}

fn apply_directive(config: &mut FakeConfig, directive: &str) {
    for part in directive.split(';') {
        let item = part.trim();
        if item.is_empty() {
            continue;
        }
        let (key, value) = item
            .split_once('=')
            .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
            .unwrap_or_else(|| (item.to_lowercase(), String::new()));
        match key.as_str() {
            "status" => config.status = str_to_status(&value),
            "reason" => config.reason = value,
            "reply" => config.reply = Some(value),
            "polls" | "polls_until_complete" => {
                if let Ok(n) = value.parse::<u64>() {
                    config.polls_until_complete = n.max(1);
                }
            }
            "latency_ms" => {
                if let Ok(n) = value.parse::<u64>() {
                    config.latency_ms = n;
                }
            }
            "confidence" => config.confidence = str_to_confidence(&value),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn status_to_str(status: CompletionStatus) -> &'static str {
    match status {
        CompletionStatus::Completed => "completed",
        CompletionStatus::Cancelled => "cancelled",
        CompletionStatus::Failed => "failed",
        CompletionStatus::Incomplete => "incomplete",
    }
}

fn str_to_status(s: &str) -> CompletionStatus {
    match s.trim().to_lowercase().as_str() {
        "completed" | "complete" => CompletionStatus::Completed,
        "cancelled" | "canceled" => CompletionStatus::Cancelled,
        "failed" | "failure" => CompletionStatus::Failed,
        _ => CompletionStatus::Incomplete,
    }
}

fn confidence_to_str(confidence: CompletionConfidence) -> &'static str {
    match confidence {
        CompletionConfidence::Exact => "exact",
        CompletionConfidence::Observed => "observed",
        CompletionConfidence::Degraded => "degraded",
    }
}

fn str_to_confidence(s: &str) -> CompletionConfidence {
    match s.trim().to_lowercase().as_str() {
        "exact" => CompletionConfidence::Exact,
        "observed" => CompletionConfidence::Observed,
        "degraded" => CompletionConfidence::Degraded,
        _ => CompletionConfidence::Exact,
    }
}

fn progress_item_kind(mode: FakeScriptMode) -> CompletionItemKind {
    match mode {
        FakeScriptMode::AnchoredSessionStability => CompletionItemKind::SessionSnapshot,
        _ => CompletionItemKind::AssistantChunk,
    }
}

fn terminal_item_kind(status: CompletionStatus, mode: FakeScriptMode) -> CompletionItemKind {
    match mode {
        FakeScriptMode::ProtocolTurn => CompletionItemKind::TurnBoundary,
        FakeScriptMode::SessionBoundary => CompletionItemKind::TurnBoundary,
        FakeScriptMode::AnchoredSessionStability => CompletionItemKind::SessionSnapshot,
        FakeScriptMode::LegacyText => CompletionItemKind::AssistantFinal,
        FakeScriptMode::StructuredResult => match status {
            CompletionStatus::Completed => CompletionItemKind::Result,
            CompletionStatus::Cancelled => CompletionItemKind::CancelInfo,
            CompletionStatus::Failed => CompletionItemKind::Error,
            CompletionStatus::Incomplete => CompletionItemKind::TurnAborted,
        },
    }
}

fn add_ms(now: &str, ms: u64) -> String {
    use chrono::{DateTime, Duration, Utc};
    DateTime::parse_from_rfc3339(now)
        .map(|dt| (dt.with_timezone(&Utc) + Duration::milliseconds(ms as i64)).to_rfc3339())
        .unwrap_or_else(|_| now.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_completion::models::TargetKind;

    fn fake_job_with_options(
        provider: &str,
        body: &str,
        options: serde_json::Map<String, Value>,
    ) -> JobRecord {
        JobRecord {
            job_id: "j1".to_string(),
            agent_name: "agent1".to_string(),
            provider: provider.to_string(),
            target_kind: TargetKind::Agent,
            request: ccb_completion::models::JobRequest {
                body: body.to_string(),
                message_type: None,
            },
            provider_options: options,
            workspace_path: None,
            provider_instance: None,
        }
    }

    #[test]
    fn test_fake_variant_mappings() {
        assert_eq!(
            FakeVariant::for_provider(PROVIDER_NAME_FAKE).source_kind,
            CompletionSourceKind::StructuredResultStream
        );
        assert_eq!(
            FakeVariant::for_provider(PROVIDER_NAME_FAKE_CODEX).source_kind,
            CompletionSourceKind::ProtocolEventStream
        );
        assert_eq!(
            FakeVariant::for_provider(PROVIDER_NAME_FAKE_CLAUDE).source_kind,
            CompletionSourceKind::SessionEventLog
        );
        assert_eq!(
            FakeVariant::for_provider(PROVIDER_NAME_FAKE_GEMINI).source_kind,
            CompletionSourceKind::SessionSnapshot
        );
        assert_eq!(
            FakeVariant::for_provider(PROVIDER_NAME_FAKE_LEGACY).source_kind,
            CompletionSourceKind::TerminalText
        );
    }

    #[test]
    fn test_fake_adapter_start_defaults() {
        let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
        let job = fake_job_with_options(PROVIDER_NAME_FAKE, "hello", serde_json::Map::new());
        let submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");
        assert_eq!(submission.provider, PROVIDER_NAME_FAKE);
        assert_eq!(submission.reply, "FAKE[agent1] hello");
        assert_eq!(
            submission.source_kind,
            CompletionSourceKind::StructuredResultStream
        );
        assert_eq!(
            submission
                .runtime_state
                .get("polls_until_complete")
                .unwrap(),
            &Value::Number(1.into())
        );
    }

    #[test]
    fn test_fake_adapter_complete_after_n_polls() {
        let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
        let mut options = serde_json::Map::new();
        options.insert("polls_until_complete".to_string(), Value::Number(3.into()));
        options.insert(
            "reply".to_string(),
            Value::String("fixed reply".to_string()),
        );
        let job = fake_job_with_options(PROVIDER_NAME_FAKE, "hello", options);
        let mut submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");

        let result = adapter.poll(&submission, "2025-01-01T00:00:00Z").unwrap();
        assert_eq!(result.items.len(), 1);
        assert!(result.decision.is_none());
        submission = result.submission;

        let result = adapter.poll(&submission, "2025-01-01T00:00:00Z").unwrap();
        assert_eq!(result.items.len(), 1);
        assert!(result.decision.is_none());
        submission = result.submission;

        let result = adapter.poll(&submission, "2025-01-01T00:00:00Z").unwrap();
        assert_eq!(result.items.len(), 1);
        let decision = result.decision.expect("terminal decision");
        assert!(decision.terminal);
        assert_eq!(decision.status, CompletionStatus::Completed);
        assert_eq!(decision.reply, "fixed reply");
    }

    #[test]
    fn test_fake_adapter_failed_status() {
        let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CODEX);
        let mut options = serde_json::Map::new();
        options.insert("status".to_string(), Value::String("failed".to_string()));
        options.insert("reason".to_string(), Value::String("api_error".to_string()));
        let job = fake_job_with_options(PROVIDER_NAME_FAKE_CODEX, "hello", options);
        let submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");
        let result = adapter.poll(&submission, "2025-01-01T00:00:00Z").unwrap();
        let decision = result.decision.expect("terminal decision");
        assert_eq!(decision.status, CompletionStatus::Failed);
        assert_eq!(decision.reason.as_deref().unwrap(), "api_error");
    }

    #[test]
    fn test_fake_adapter_cancelled_status() {
        let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CLAUDE);
        let mut options = serde_json::Map::new();
        options.insert("status".to_string(), Value::String("cancelled".to_string()));
        let job = fake_job_with_options(PROVIDER_NAME_FAKE_CLAUDE, "hello", options);
        let submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");
        let result = adapter.poll(&submission, "2025-01-01T00:00:00Z").unwrap();
        let decision = result.decision.expect("terminal decision");
        assert_eq!(decision.status, CompletionStatus::Cancelled);
    }

    #[test]
    fn test_fake_backends() {
        let bs = backends();
        assert_eq!(bs.len(), TEST_DOUBLE_PROVIDER_NAMES.len());
        for backend in bs {
            assert!(TEST_DOUBLE_PROVIDER_NAMES.contains(&backend.provider()));
            assert!(backend
                .manifest
                .supports_runtime_mode(&RuntimeMode::PaneBacked));
        }
    }
}
