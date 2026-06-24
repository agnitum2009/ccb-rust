use ccbr_completion::models::{
    CompletionConfidence, CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::manifest::RuntimeMode;
use ccbr_providers::execution::{ExecutionAdapter, ProviderRuntimeContext};
use ccbr_providers::providers::copilot::{
    backend, extract_reply_for_req, find_project_session_file, is_done_text, load_project_session,
    manifest, wrap_copilot_prompt, CopilotExecutionAdapter, CopilotProjectSession, CopilotRequest,
    CopilotResult, PROVIDER_NAME,
};
use serde_json::Value;

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

#[test]
fn test_manifest_capabilities_and_profiles() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(!m.supports_resume);
    assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));

    let pane = m.completion_manifest_for(&RuntimeMode::PaneBacked).unwrap();
    assert_eq!(pane.provider, PROVIDER_NAME);
    assert_eq!(pane.runtime_mode, "pane-backed");
    assert!(pane.poll_interval_ms > 0);
    assert!(pane.timeout_ms > 0);
}

#[test]
fn test_backend_includes_binding_and_launcher() {
    let b = backend();
    assert_eq!(b.provider(), PROVIDER_NAME);
    assert!(b.session_binding.is_some());
    let binding = b.session_binding.unwrap();
    assert_eq!(binding.session_id_attr, "copilot_session_id");
    assert_eq!(binding.session_path_attr, "copilot_session_path");
    assert!(b.runtime_launcher.is_some());
}

#[test]
fn test_execution_adapter_provider_name() {
    let adapter = CopilotExecutionAdapter;
    assert_eq!(adapter.provider(), PROVIDER_NAME);
}

#[test]
fn test_start_creates_active_submission() {
    let adapter = CopilotExecutionAdapter;
    let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
    let submission = adapter.start(&job, None, &fake_now());

    assert_eq!(submission.job_id, "j1");
    assert_eq!(submission.agent_name, "agent1");
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::ProtocolEventStream
    );
    assert!(!submission.is_terminal());

    let state = &submission.runtime_state;
    assert_eq!(state.get("mode").unwrap(), "active");
    assert!(state
        .get("request_anchor")
        .unwrap()
        .as_str()
        .unwrap()
        .starts_with("<<BEGIN:"));
    assert_eq!(state.get("prompt_sent").unwrap(), false);
    assert!(state
        .get("prompt_text")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("<<DONE:"));
}

#[test]
fn test_poll_returns_none_without_done_marker() {
    let adapter = CopilotExecutionAdapter;
    let job = JobRecord::new("j2", "agent2", PROVIDER_NAME);
    let submission = adapter.start(&job, None, &fake_now());
    assert!(adapter.poll(&submission, &fake_now()).is_none());
}

#[test]
fn test_poll_emits_terminal_decision_on_done_marker() {
    let adapter = CopilotExecutionAdapter;
    let job = JobRecord::new("j3", "agent3", PROVIDER_NAME);
    let mut submission = adapter.start(&job, None, &fake_now());

    let req_id = ccbr_provider_core::protocol::make_req_id(&job.job_id);
    let reply = format!(
        "{}\nhello copilot\n{}",
        ccbr_provider_core::protocol::request_anchor_for_job(&job.job_id),
        ccbr_provider_core::protocol::done_marker(&req_id)
    );
    submission
        .runtime_state
        .insert("reply_buffer".to_string(), Value::String(reply));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::AssistantFinal);
    assert_eq!(
        result.items[0]
            .payload
            .get("reply")
            .unwrap()
            .as_str()
            .unwrap(),
        "hello copilot"
    );

    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.confidence, Some(CompletionConfidence::Exact));
    assert_eq!(decision.reply, "hello copilot");
}

#[test]
fn test_wrap_copilot_prompt_format() {
    let wrapped = wrap_copilot_prompt("do the thing", "req-12345678");
    assert!(wrapped.contains("req-12345678"));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.contains("IMPORTANT:"));
    assert!(wrapped.contains("<<DONE:"));
    assert!(wrapped.ends_with('\n'));
}

#[test]
fn test_wrap_copilot_prompt_strips_trailing_whitespace() {
    let wrapped = wrap_copilot_prompt("  test  \n\n", "req-12345678");
    assert!(wrapped.contains("  test"));
}

#[test]
fn test_extract_reply_for_req_basic() {
    let text = "<<BEGIN:req-12345678>>\nsome preamble\n<<DONE:req-12345678>>";
    let reply = extract_reply_for_req(text, "req-12345678");
    assert_eq!(reply, "some preamble");
}

#[test]
fn test_extract_reply_for_req_empty_on_wrong_id() {
    let text = "content\n<<DONE:req-87654321>>";
    let reply = extract_reply_for_req(text, "req-12345678");
    assert_eq!(reply, "");
}

#[test]
fn test_is_done_text() {
    let text = "some text <<DONE:req-12345678>> more";
    assert!(is_done_text(text));
}

#[test]
fn test_request_and_result_defaults() {
    let req = CopilotRequest {
        client_id: "client-1".into(),
        work_dir: "/tmp/test".into(),
        timeout_s: 60.0,
        quiet: false,
        message: "hello".into(),
        req_id: None,
        caller: "claude".into(),
    };
    assert_eq!(req.client_id, "client-1");
    assert_eq!(req.caller, "claude");
    assert!(req.req_id.is_none());

    let result = CopilotResult {
        exit_code: 0,
        reply: "test reply".into(),
        req_id: "abc123".into(),
        session_key: "copilot:xyz".into(),
        done_seen: true,
        done_ms: Some(1500),
        anchor_seen: false,
        fallback_scan: false,
        anchor_ms: None,
    };
    assert_eq!(result.exit_code, 0);
    assert!(result.done_seen);
    assert!(!result.anchor_seen);
}

#[test]
fn test_find_project_session_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let session_path = tmp.path().join(".copilot-session");
    std::fs::write(&session_path, "{}").unwrap();

    let found = find_project_session_file(tmp.path(), None).unwrap();
    assert_eq!(found, session_path);
}

#[test]
fn test_load_project_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    let session_path = tmp.path().join(".copilot-session");
    std::fs::write(&session_path, r#"{"copilot_session_id":"s1"}"#).unwrap();

    let session = load_project_session(tmp.path(), None).unwrap();
    assert_eq!(session.copilot_session_id(), "s1");
    assert_eq!(session.session_file, session_path);
}

#[test]
fn test_project_session_default() {
    let session = CopilotProjectSession::default();
    assert!(session.copilot_session_id().is_empty());
    assert!(session.copilot_session_path().is_empty());
}

#[test]
fn test_start_with_runtime_context_is_ignored() {
    let adapter = CopilotExecutionAdapter;
    let job = JobRecord::new("j4", "agent4", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        agent_name: "agent4".to_string(),
        workspace_path: Some("/tmp/ws".to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    assert_eq!(submission.provider, PROVIDER_NAME);
}
