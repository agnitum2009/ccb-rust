use std::io::Write;

use ccb_completion::models::{CompletionItemKind, CompletionStatus, JobRecord};
use ccb_provider_core::contracts::LaunchMode;
use ccb_providers::{
    build_default_backend_registry, build_default_execution_registry,
    execution::{ExecutionAdapter, PersistedExecutionState},
    providers::codex::{
        backend, build_runtime_launcher, build_session_binding, CodexCommunicator,
        CodexExecutionAdapter,
    },
};

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn job_with_body(job_id: &str, agent_name: &str, body: &str) -> JobRecord {
    JobRecord {
        job_id: job_id.to_string(),
        agent_name: agent_name.to_string(),
        provider: "codex".to_string(),
        target_kind: ccb_completion::models::TargetKind::Agent,
        request: ccb_completion::models::JobRequest {
            body: body.to_string(),
            message_type: None,
        },
        provider_options: serde_json::Map::new(),
        workspace_path: None,
        provider_instance: None,
    }
}

#[test]
fn test_codex_manifest_and_backend_registry() {
    let backend = backend();
    assert_eq!(backend.provider(), "codex");
    assert!(backend
        .manifest
        .supports_runtime_mode(&ccb_provider_core::manifest::RuntimeMode::PaneBacked));

    let registry = build_default_backend_registry();
    assert!(registry.get("codex").is_some());
}

#[test]
fn test_codex_manifest_capabilities() {
    let backend = backend();
    assert!(backend.manifest.supports_resume);
    assert!(backend.manifest.supports_permission_auto);
    assert!(backend.manifest.supports_stream_watch);
    assert!(!backend.manifest.supports_subagents);
    assert!(backend.manifest.supports_workspace_attach);
}

#[test]
fn test_codex_execution_registry_contains_adapter() {
    let registry = build_default_execution_registry();
    assert!(registry.get("codex").is_some());
}

#[test]
fn test_codex_session_binding_and_launcher() {
    let binding = build_session_binding();
    assert_eq!(binding.provider, "codex");
    assert_eq!(binding.session_id_attr, "codex_session_id");
    assert_eq!(binding.session_path_attr, "codex_session_path");

    let launcher = build_runtime_launcher();
    assert_eq!(launcher.provider, "codex");
    assert_eq!(launcher.launch_mode, LaunchMode::CodexTmux);
}

#[test]
fn test_codex_execution_adapter_start_active() {
    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j1", "agent1", "hello codex");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/tmp/ws".to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: None,
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    assert_eq!(submission.provider, "codex");
    assert_eq!(submission.job_id, "j1");
    let state = &submission.runtime_state;
    assert_eq!(state.get("mode").unwrap(), "active");
    assert_eq!(state.get("delivery_state").unwrap(), "pending_anchor");
    assert_eq!(
        state.get("session_path").unwrap().as_str().unwrap(),
        "/tmp/ws/codex-session.jsonl"
    );
    assert_eq!(state.get("delivery_target_pane_id").unwrap(), "%12");
    assert!(state
        .get("request_anchor")
        .unwrap()
        .as_str()
        .unwrap()
        .starts_with("<<BEGIN:"));
}

#[test]
fn test_codex_export_runtime_state() {
    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j1", "agent1", "hello codex");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/tmp/ws".to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: None,
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    let exported = adapter.export_runtime_state(&submission).unwrap();
    assert_eq!(exported.get("mode").unwrap(), "active");
    assert!(exported.contains_key("request_anchor"));
    assert!(exported.contains_key("session_path"));
    assert!(exported.contains_key("delivery_state"));
}

#[test]
fn test_codex_execution_adapter_poll_full_turn() {
    let tmp = tempfile::TempDir::new().unwrap();
    let log_path = tmp.path().join("codex-session.jsonl");
    let request_anchor = ccb_provider_core::protocol::request_anchor_for_job("j1");
    let req_id = ccb_provider_core::protocol::make_req_id("j1");

    let mut file = std::fs::File::create(&log_path).unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"{prefix} {anchor}"}},"timestamp":"t1"}}"#,
        prefix = ccb_provider_core::protocol::REQ_ID_PREFIX,
        anchor = request_anchor
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"working"}}]}},"timestamp":"t2"}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"done"}}]}},"timestamp":"t3"}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"task_complete","last_agent_message":"<<DONE:{req_id}>> final answer"}},"timestamp":"t4"}}"#
    )
    .unwrap();

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j1", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: Some(log_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    let result = adapter.poll(&submission, &fake_now()).unwrap();

    assert_eq!(result.items.len(), 4);
    assert_eq!(result.items[0].kind, CompletionItemKind::AnchorSeen);
    assert_eq!(result.items[1].kind, CompletionItemKind::AssistantChunk);
    assert_eq!(result.items[2].kind, CompletionItemKind::AssistantChunk);
    assert_eq!(result.items[3].kind, CompletionItemKind::TurnBoundary);

    let decision = result.decision.unwrap();
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reason.as_deref().unwrap(), "task_complete");
    assert!(!decision.reply.is_empty());
    assert!(result
        .submission
        .runtime_state
        .get("anchor_seen")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[test]
fn test_codex_execution_adapter_poll_turn_aborted() {
    let tmp = tempfile::TempDir::new().unwrap();
    let log_path = tmp.path().join("codex-session.jsonl");
    let request_anchor = ccb_provider_core::protocol::request_anchor_for_job("j2");

    let mut file = std::fs::File::create(&log_path).unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"{prefix} {anchor}"}},"timestamp":"t1"}}"#,
        prefix = ccb_provider_core::protocol::REQ_ID_PREFIX,
        anchor = request_anchor
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"turn_aborted","reason":"user_cancelled","message":"user cancelled"}},"timestamp":"t2"}}"#
    )
    .unwrap();

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j2", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: Some(log_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    let result = adapter.poll(&submission, &fake_now()).unwrap();

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.items[1].kind, CompletionItemKind::TurnAborted);
    assert_eq!(
        result.items[1]
            .payload
            .get("status")
            .unwrap()
            .as_str()
            .unwrap(),
        "cancelled"
    );

    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Cancelled);
}

#[test]
fn test_codex_execution_adapter_poll_no_log_returns_none() {
    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j3", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/nonexistent".to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    assert!(adapter.poll(&submission, &fake_now()).is_none());
}

#[test]
fn test_codex_execution_adapter_resume_requires_active_mode() {
    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j4", "agent1", "hello");
    let mut submission = adapter.start(&job, None, &fake_now());
    submission
        .runtime_state
        .insert("mode".to_string(), serde_json::json!("passive"));
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/tmp".to_string()),
        ..Default::default()
    };
    let persisted = PersistedExecutionState::new(submission.clone(), None, false, fake_now());
    assert!(adapter
        .resume(&job, &submission, Some(&ctx), &persisted, &fake_now())
        .is_none());
}

#[test]
fn test_codex_communicator_wrap_and_fifo_mock() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fifo_path = tmp.path().join("input.fifo");
    // Use a regular file as a FIFO stand-in for unit testing.
    std::fs::File::create(&fifo_path).unwrap();
    let comm = CodexCommunicator::new(&fifo_path);
    let wrapped = comm.wrap_turn_prompt("hello", "req-12345678");
    assert!(wrapped.contains("<<BEGIN:req-12345678>>"));
    assert!(wrapped.contains("hello"));
    assert!(wrapped.contains("<<DONE:req-12345678>>"));

    comm.send_async("ping").unwrap();
    let contents = std::fs::read_to_string(&fifo_path).unwrap();
    assert_eq!(contents.trim(), "ping");
}

#[test]
fn test_codex_log_entry_extraction_variants() {
    use ccb_providers::providers::codex::CodexExecutionAdapter;
    // The internal extraction is exercised through the public adapter poll tests above.
    // This test verifies that the provider name is normalized.
    let adapter = CodexExecutionAdapter;
    assert_eq!(adapter.provider(), "codex");
}

fn write_codex_session(
    dir: &std::path::Path,
    session_path: impl AsRef<std::path::Path>,
    extra: serde_json::Map<String, serde_json::Value>,
) -> std::path::PathBuf {
    let session_path = session_path.as_ref();
    let mut data = serde_json::json!({
        "codex_session_path": session_path.to_string_lossy().to_string(),
        "work_dir": dir.to_string_lossy().to_string(),
    });
    if let Some(obj) = data.as_object_mut() {
        for (k, v) in extra {
            obj.insert(k, v);
        }
    }
    let path = dir.join(".codex-session");
    std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
    path
}

#[test]
fn test_codex_delivery_acceptance_guard_times_out() {
    let tmp = tempfile::TempDir::new().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir(&workspace).unwrap();
    let log_path = workspace.join("codex-session.jsonl");
    std::fs::File::create(&log_path).unwrap();
    write_codex_session(&workspace, &log_path, serde_json::Map::new());

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j-delivery", "agent1", "hello codex");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(workspace.to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: None,
        ..Default::default()
    };
    let mut submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    submission
        .runtime_state
        .insert("delivery_timeout_s".to_string(), serde_json::json!(1.0));

    let result = adapter
        .poll(&submission, "2025-01-01T00:05:00Z")
        .expect("guard should fire");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::Error);
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert_eq!(
        decision.reason.as_deref(),
        Some("codex_prompt_delivery_failed")
    );
    assert_eq!(
        decision
            .diagnostics
            .get("delivery_failure_kind")
            .and_then(|v| v.as_str()),
        Some("delivery_anchor_missing")
    );
    assert_eq!(
        result.submission.runtime_state.get("delivery_state"),
        Some(&serde_json::json!("failed"))
    );
    assert_eq!(
        result.submission.runtime_state.get("mode"),
        Some(&serde_json::json!("passive"))
    );
}

#[test]
fn test_codex_anchor_fallback_log_scanning() {
    let tmp = tempfile::TempDir::new().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir(&workspace).unwrap();
    let session_root = workspace.join("sessions");
    std::fs::create_dir(&session_root).unwrap();

    let primary_log = session_root.join("primary.jsonl");
    std::fs::File::create(&primary_log).unwrap();
    let mut extra = serde_json::Map::new();
    extra.insert(
        "codex_session_root".to_string(),
        serde_json::json!(session_root.to_string_lossy().to_string()),
    );
    write_codex_session(&workspace, &primary_log, extra);

    let request_anchor = ccb_provider_core::protocol::request_anchor_for_job("j-fallback");
    let fallback_log = session_root.join("550e8400-e29b-41d4-a716-446655440000.jsonl");
    let cwd = workspace.canonicalize().unwrap();
    let mut file = std::fs::File::create(&fallback_log).unwrap();
    writeln!(
        file,
        r#"{{"type":"session_meta","payload":{{"cwd":"{cwd}","session":{{"id":"550e8400-e29b-41d4-a716-446655440000"}}}}}}"#,
        cwd = cwd.to_string_lossy()
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"{prefix} {anchor}"}},"timestamp":"t1"}}"#,
        prefix = ccb_provider_core::protocol::REQ_ID_PREFIX,
        anchor = request_anchor
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"fallback reply"}}]}},"timestamp":"t2"}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"task_complete","last_agent_message":"<<DONE:req-00000000>> fallback reply"}},"timestamp":"t3"}}"#
    )
    .unwrap();

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j-fallback", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(workspace.to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: None,
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    let result = adapter.poll(&submission, &fake_now()).unwrap();

    let kinds: Vec<_> = result.items.iter().map(|i| i.kind).collect();
    assert!(
        kinds.contains(&CompletionItemKind::SessionRotate),
        "expected session rotation to fallback log, got {:?}",
        kinds
    );
    assert!(
        kinds.contains(&CompletionItemKind::AnchorSeen),
        "expected anchor_seen item, got {:?}",
        kinds
    );
    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert!(decision.reply.contains("fallback reply"));
}

#[test]
fn test_codex_session_refresh_picks_new_log() {
    let tmp = tempfile::TempDir::new().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir(&workspace).unwrap();
    let log1 = workspace.join("codex-session-1.jsonl");
    std::fs::File::create(&log1).unwrap();
    write_codex_session(&workspace, &log1, serde_json::Map::new());

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j-refresh", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(workspace.to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: None,
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());

    let log2 = workspace.join("codex-session-2.jsonl");
    let request_anchor = ccb_provider_core::protocol::request_anchor_for_job("j-refresh");
    let req_id = ccb_provider_core::protocol::make_req_id("j-refresh");
    let mut file = std::fs::File::create(&log2).unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"{prefix} {anchor}"}},"timestamp":"t1"}}"#,
        prefix = ccb_provider_core::protocol::REQ_ID_PREFIX,
        anchor = request_anchor
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"task_complete","last_agent_message":"<<DONE:{req_id}>> refreshed answer"}},"timestamp":"t2"}}"#
    )
    .unwrap();

    // Rewrite the session file to point at the new log.
    write_codex_session(&workspace, &log2, serde_json::Map::new());

    let result = adapter.poll(&submission, &fake_now()).unwrap();
    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert!(decision.reply.contains("refreshed answer"));
}

#[test]
fn test_codex_terminal_decision_includes_diagnostics() {
    let tmp = tempfile::TempDir::new().unwrap();
    let workspace = tmp.path().join("ws");
    std::fs::create_dir(&workspace).unwrap();
    let log_path = workspace.join("codex-session.jsonl");
    let request_anchor = ccb_provider_core::protocol::request_anchor_for_job("j-diag");
    let mut file = std::fs::File::create(&log_path).unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"{prefix} {anchor}"}},"timestamp":"t1"}}"#,
        prefix = ccb_provider_core::protocol::REQ_ID_PREFIX,
        anchor = request_anchor
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"event_msg","payload":{{"type":"task_complete","last_agent_message":""}},"timestamp":"t2"}}"#
    )
    .unwrap();
    write_codex_session(&workspace, &log_path, serde_json::Map::new());

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j-diag", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(workspace.to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: None,
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    let result = adapter.poll(&submission, &fake_now()).unwrap();

    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert!(decision.reply.is_empty());
    assert_eq!(
        decision.diagnostics.get("reason").and_then(|v| v.as_str()),
        Some("task_complete")
    );
    assert_eq!(
        decision
            .diagnostics
            .get("reply_empty")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        decision
            .diagnostics
            .get("anchor_seen")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}
