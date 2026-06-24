use ccbr_completion::models::{
    CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord, TargetKind,
};
use ccbr_provider_core::manifest::RuntimeMode;
use ccbr_providers::{
    build_default_backend_registry, build_default_execution_registry,
    execution::ExecutionAdapter,
    providers::fake::{
        backend, execution_adapters, manifest, FakeExecutionAdapter, PROVIDER_NAME_FAKE,
        PROVIDER_NAME_FAKE_CLAUDE, PROVIDER_NAME_FAKE_CODEX, PROVIDER_NAME_FAKE_GEMINI,
        PROVIDER_NAME_FAKE_LEGACY, TEST_DOUBLE_PROVIDER_NAMES,
    },
};
use serde_json::Value;

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn job_with_options(
    provider: &str,
    body: &str,
    options: serde_json::Map<String, Value>,
) -> JobRecord {
    JobRecord {
        job_id: "j1".to_string(),
        agent_name: "agent1".to_string(),
        provider: provider.to_string(),
        target_kind: TargetKind::Agent,
        request: ccbr_completion::models::JobRequest {
            body: body.to_string(),
            message_type: None,
        },
        provider_options: options,
        workspace_path: None,
        provider_instance: None,
    }
}

#[test]
fn test_fake_execution_adapters_are_registered() {
    let registry = build_default_execution_registry();
    for name in TEST_DOUBLE_PROVIDER_NAMES {
        assert!(
            registry.get(name).is_some(),
            "missing execution adapter {name}"
        );
    }
}

#[test]
fn test_fake_backends_are_registered() {
    let registry = build_default_backend_registry();
    for name in TEST_DOUBLE_PROVIDER_NAMES {
        assert!(registry.get(name).is_some(), "missing backend {name}");
    }
}

#[test]
fn test_fake_manifest_variants() {
    for name in TEST_DOUBLE_PROVIDER_NAMES {
        let m = manifest(name);
        assert_eq!(m.provider, *name);
        assert!(m.supports_resume);
        assert!(m.supports_permission_auto);
        assert!(m.supports_stream_watch);
        assert!(!m.supports_subagents);
        assert!(m.supports_workspace_attach);
        assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
        if *name == PROVIDER_NAME_FAKE {
            assert!(m.supports_runtime_mode(&RuntimeMode::Headless));
        }
    }
}

#[test]
fn test_fake_backend_variants() {
    let adapters = execution_adapters();
    assert_eq!(adapters.len(), TEST_DOUBLE_PROVIDER_NAMES.len());

    for name in TEST_DOUBLE_PROVIDER_NAMES {
        let b = backend(name);
        assert_eq!(b.provider(), *name);
        assert!(b.execution_adapter.is_none());
        assert!(b.session_binding.is_none());
        assert!(b.runtime_launcher.is_none());
    }
}

#[test]
fn test_fake_variant_source_kinds() {
    assert_eq!(
        FakeExecutionAdapter::new(PROVIDER_NAME_FAKE).source_kind(),
        CompletionSourceKind::StructuredResultStream
    );
    assert_eq!(
        FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CODEX).source_kind(),
        CompletionSourceKind::ProtocolEventStream
    );
    assert_eq!(
        FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CLAUDE).source_kind(),
        CompletionSourceKind::SessionEventLog
    );
    assert_eq!(
        FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_GEMINI).source_kind(),
        CompletionSourceKind::SessionSnapshot
    );
    assert_eq!(
        FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_LEGACY).source_kind(),
        CompletionSourceKind::TerminalText
    );
}

#[test]
fn test_fake_adapter_start_defaults() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
    let job = job_with_options(PROVIDER_NAME_FAKE, "hello", serde_json::Map::new());
    let submission = adapter.start(&job, None, &fake_now());

    assert_eq!(submission.provider, PROVIDER_NAME_FAKE);
    assert_eq!(submission.reply, "FAKE[agent1] hello");
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::StructuredResultStream
    );
    assert_eq!(submission.status, CompletionStatus::Incomplete);
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
    let job = job_with_options(PROVIDER_NAME_FAKE, "hello", options);
    let mut submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::AssistantChunk);
    assert!(result.decision.is_none());
    submission = result.submission;

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert!(result.decision.is_none());
    submission = result.submission;

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::Result);
    let decision = result.decision.expect("terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "fixed reply");
    assert_eq!(result.submission.status, CompletionStatus::Completed);
}

#[test]
fn test_fake_codex_adapter_complete_after_n_polls() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CODEX);
    let mut options = serde_json::Map::new();
    options.insert("polls_until_complete".to_string(), Value::Number(2.into()));
    let job = job_with_options(PROVIDER_NAME_FAKE_CODEX, "hello", options);
    let mut submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::AssistantChunk);
    assert!(result.decision.is_none());
    submission = result.submission;

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::TurnBoundary);
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Completed);
}

#[test]
fn test_fake_gemini_adapter_complete_after_n_polls() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_GEMINI);
    let mut options = serde_json::Map::new();
    options.insert("polls_until_complete".to_string(), Value::Number(2.into()));
    options.insert(
        "reply".to_string(),
        Value::String("gemini reply".to_string()),
    );
    let job = job_with_options(PROVIDER_NAME_FAKE_GEMINI, "hello", options);
    let mut submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::SessionSnapshot);
    assert!(result.decision.is_none());
    submission = result.submission;

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::SessionSnapshot);
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "gemini reply");
}

#[test]
fn test_fake_legacy_adapter_complete_after_n_polls() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_LEGACY);
    let mut options = serde_json::Map::new();
    options.insert("polls_until_complete".to_string(), Value::Number(1.into()));
    options.insert(
        "reply".to_string(),
        Value::String("legacy reply".to_string()),
    );
    let job = job_with_options(PROVIDER_NAME_FAKE_LEGACY, "hello", options);
    let submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::AssistantFinal);
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "legacy reply");
}

#[test]
fn test_fake_adapter_failed_status() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CODEX);
    let mut options = serde_json::Map::new();
    options.insert("status".to_string(), Value::String("failed".to_string()));
    options.insert("reason".to_string(), Value::String("api_error".to_string()));
    let job = job_with_options(PROVIDER_NAME_FAKE_CODEX, "hello", options);
    let submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert_eq!(decision.reason.as_deref().unwrap(), "api_error");
}

#[test]
fn test_fake_adapter_cancelled_status() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE_CLAUDE);
    let mut options = serde_json::Map::new();
    options.insert("status".to_string(), Value::String("cancelled".to_string()));
    let job = job_with_options(PROVIDER_NAME_FAKE_CLAUDE, "hello", options);
    let submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Cancelled);
}

#[test]
fn test_fake_adapter_directive_string() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
    let mut options = serde_json::Map::new();
    options.insert(
        "directive".to_string(),
        Value::String("status=failed;reason=crash;reply=broken;polls=2".to_string()),
    );
    let job = job_with_options(PROVIDER_NAME_FAKE, "hello", options);
    let mut submission = adapter.start(&job, None, &fake_now());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert!(result.decision.is_none());
    submission = result.submission;

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert_eq!(decision.reason.as_deref().unwrap(), "crash");
    assert_eq!(decision.reply, "broken");
}

#[test]
fn test_fake_adapter_resume() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
    let job = job_with_options(PROVIDER_NAME_FAKE, "hello", serde_json::Map::new());
    let submission = adapter.start(&job, None, &fake_now());
    let resumed = adapter.resume(
        &job,
        &submission,
        None,
        &ccbr_providers::execution::PersistedExecutionState::new(
            submission.clone(),
            None,
            true,
            fake_now(),
        ),
        &fake_now(),
    );
    assert!(resumed.is_some());
    assert_eq!(resumed.unwrap().job_id, "j1");
}

#[test]
fn test_fake_adapter_export_runtime_state() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
    let job = job_with_options(PROVIDER_NAME_FAKE, "hello", serde_json::Map::new());
    let submission = adapter.start(&job, None, &fake_now());
    let exported = adapter.export_runtime_state(&submission).unwrap();
    assert!(exported.contains_key("poll_count"));
    assert!(exported.contains_key("polls_until_complete"));
    assert!(exported.contains_key("reply"));
}

#[test]
fn test_fake_adapter_poll_after_terminal_returns_none() {
    let adapter = FakeExecutionAdapter::new(PROVIDER_NAME_FAKE);
    let job = job_with_options(PROVIDER_NAME_FAKE, "hello", serde_json::Map::new());
    let submission = adapter.start(&job, None, &fake_now());
    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert!(result.decision.is_some());

    let next = adapter.poll(&result.submission, &fake_now());
    assert!(next.is_none());
}
