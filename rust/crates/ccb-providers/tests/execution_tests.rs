use ccb_completion::models::{
    CompletionConfidence, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_providers::{
    build_default_execution_registry,
    execution::{
        passive_submission, ExecutionService, ProviderExecutionRegistry, ProviderPollResult,
        ProviderRuntimeContext, ProviderSubmission,
    },
};

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

#[test]
fn test_execution_registry_case_insensitive() {
    let mut registry = ProviderExecutionRegistry::new();
    registry.register(Box::new(
        ccb_providers::providers::claude::ClaudeExecutionAdapter,
    ));
    assert!(registry.get("claude").is_some());
    assert!(registry.get("CLAUDE").is_some());
}

#[test]
#[should_panic(expected = "duplicate execution adapter")]
fn test_execution_registry_duplicate_panics() {
    let mut registry = ProviderExecutionRegistry::new();
    registry.register(Box::new(
        ccb_providers::providers::claude::ClaudeExecutionAdapter,
    ));
    registry.register(Box::new(
        ccb_providers::providers::claude::ClaudeExecutionAdapter,
    ));
}

#[test]
fn test_execution_service_start_and_poll() {
    let registry = build_default_execution_registry();
    let mut service = ExecutionService::new(registry, fake_now, None);
    let job = JobRecord::new("j1", "agent1", "claude");
    let submission = service.start(&job, None);
    assert!(submission.is_some());
    let updates = service.poll();
    // Minimal stubs return no poll results.
    assert!(updates.is_empty());
}

#[test]
fn test_execution_service_cancel() {
    let registry = build_default_execution_registry();
    let mut service = ExecutionService::new(registry, fake_now, None);
    let job = JobRecord::new("j1", "agent1", "claude");
    service.start(&job, None);
    service.cancel("j1");
    assert!(service.active_runtime_snapshots().is_empty());
}

#[test]
fn test_passive_submission() {
    let job = JobRecord::new("j2", "agent2", "gemini");
    let sub = passive_submission(
        &job,
        "gemini",
        "2025-01-01T00:00:00Z",
        CompletionSourceKind::ProtocolEventStream,
        "no_runtime",
    );
    assert_eq!(sub.job_id, "j2");
    assert_eq!(sub.provider, "gemini");
    assert_eq!(sub.runtime_state.get("mode").unwrap(), "passive");
    assert!(!sub.is_terminal());
}

#[test]
fn test_provider_poll_result_terminal_decision_validation() {
    let job = JobRecord::new("j3", "agent3", "claude");
    let submission = ProviderSubmission::new(
        &job,
        "claude",
        "2025-01-01T00:00:00Z",
        CompletionSourceKind::ProtocolEventStream,
    );
    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("done".to_string()),
        confidence: Some(CompletionConfidence::Observed),
        reply: "hello".to_string(),
        anchor_seen: true,
        reply_started: true,
        reply_stable: true,
        provider_turn_ref: Some("j3".to_string()),
        source_cursor: None,
        finished_at: Some("2025-01-01T00:00:01Z".to_string()),
        diagnostics: Default::default(),
    };
    let result = ProviderPollResult::new(submission, Vec::new(), Some(decision));
    assert!(result.decision.is_some());
}

#[test]
#[should_panic(expected = "provider poll decisions must be terminal")]
fn test_provider_poll_result_rejects_non_terminal_decision() {
    let job = JobRecord::new("j4", "agent4", "claude");
    let submission = ProviderSubmission::new(
        &job,
        "claude",
        "2025-01-01T00:00:00Z",
        CompletionSourceKind::ProtocolEventStream,
    );
    let decision = CompletionDecision {
        terminal: false,
        status: CompletionStatus::Incomplete,
        reason: None,
        confidence: None,
        reply: String::new(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: None,
        source_cursor: None,
        finished_at: None,
        diagnostics: Default::default(),
    };
    let _result = ProviderPollResult::new(submission, Vec::new(), Some(decision));
}

#[test]
fn test_runtime_context_round_trip() {
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/tmp/ws".to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: None,
        session_ref: Some("session.json".to_string()),
        runtime_pid: Some(1234),
        runtime_health: Some("healthy".to_string()),
        runtime_binding_source: None,
    };
    let json = serde_json::to_string(&ctx).unwrap();
    let restored: ProviderRuntimeContext = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.workspace_path, Some("/tmp/ws".to_string()));
    assert_eq!(restored.runtime_pid, Some(1234));
}

struct FakeEmittingAdapter;

impl ccb_providers::execution::ExecutionAdapter for FakeEmittingAdapter {
    fn provider(&self) -> &str {
        "fake"
    }

    fn start(
        &self,
        job: &JobRecord,
        _context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        ProviderSubmission::new(job, "fake", now, CompletionSourceKind::ProtocolEventStream)
    }

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        let item = CompletionItem::new(
            CompletionItemKind::Result,
            now.to_string(),
            ccb_completion::models::CompletionCursor::new(
                CompletionSourceKind::ProtocolEventStream,
                now,
            ),
            submission.provider.clone(),
            submission.agent_name.clone(),
            submission.job_id.clone(),
        )
        .ok()?;
        let decision = CompletionDecision {
            terminal: true,
            status: CompletionStatus::Completed,
            reason: Some("done".to_string()),
            confidence: Some(CompletionConfidence::Observed),
            reply: "finished".to_string(),
            anchor_seen: true,
            reply_started: true,
            reply_stable: true,
            provider_turn_ref: Some(submission.job_id.clone()),
            source_cursor: Some(item.cursor.clone()),
            finished_at: Some(now.to_string()),
            diagnostics: Default::default(),
        };
        Some(ProviderPollResult::new(
            submission.clone(),
            vec![item],
            Some(decision),
        ))
    }
}

#[test]
fn test_execution_service_poll_emits_update_and_finishes() {
    let mut registry = ProviderExecutionRegistry::new();
    registry.register(Box::new(FakeEmittingAdapter));
    let mut service = ExecutionService::new(registry, fake_now, None);
    let job = JobRecord::new("j5", "agent5", "fake");
    service.start(&job, None);
    let updates = service.poll();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].job_id, "j5");
    assert!(updates[0].decision.is_some());
    // Terminal decision should remove from active.
    assert!(service.active_runtime_snapshots().is_empty());
}
