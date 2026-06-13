use std::path::PathBuf;

use ccb_completion::models::{
    CompletionConfidence, CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::manifest::RuntimeMode;
use ccb_providers::execution::{ExecutionAdapter, ProviderRuntimeContext};
use ccb_providers::providers::kimi::{
    backend, clean_kimi_pane_reply, extract_reply_for_req, find_project_session_file, is_done_text,
    kimi_context_path, load_project_session, looks_like_kimi_input_box_line,
    looks_like_kimi_non_answer, manifest, wrap_kimi_prompt, KimiExecutionAdapter,
    KimiProjectSession, KimiRequest, KimiResult, PROVIDER_NAME,
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
    assert_eq!(binding.session_id_attr, "kimi_session_id");
    assert_eq!(binding.session_path_attr, "kimi_session_path");
    assert!(b.runtime_launcher.is_some());
}

#[test]
fn test_execution_adapter_provider_name() {
    let adapter = KimiExecutionAdapter;
    assert_eq!(adapter.provider(), PROVIDER_NAME);
}

#[test]
fn test_start_creates_active_submission_with_context_pointer() {
    let adapter = KimiExecutionAdapter;
    let job = JobRecord::new("j1", "slot1_claude", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        agent_name: "slot1_claude".to_string(),
        workspace_path: Some("/repo".to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());

    assert_eq!(submission.job_id, "j1");
    assert_eq!(submission.agent_name, "slot1_claude");
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::ProtocolEventStream
    );
    assert!(!submission.is_terminal());

    let state = &submission.runtime_state;
    let context_path =
        "/repo/.ccb/agents/slot1_claude/provider-state/kimi/home/CCB_KIMI_CONTEXT.md";
    assert_eq!(state.get("mode").unwrap(), "active");
    assert_eq!(state.get("kimi_context_path").unwrap(), context_path);
    assert_eq!(state.get("kimi_context_projection").unwrap(), "file");
    assert!(state
        .get("prompt_text")
        .unwrap()
        .as_str()
        .unwrap()
        .contains(context_path));
}

#[test]
fn test_poll_returns_none_without_done_marker() {
    let adapter = KimiExecutionAdapter;
    let job = JobRecord::new("j2", "agent2", PROVIDER_NAME);
    let submission = adapter.start(&job, None, &fake_now());
    assert!(adapter.poll(&submission, &fake_now()).is_none());
}

#[test]
fn test_poll_emits_terminal_decision_on_done_marker() {
    let adapter = KimiExecutionAdapter;
    let job = JobRecord::new("j3", "agent3", PROVIDER_NAME);
    let mut submission = adapter.start(&job, None, &fake_now());

    let req_id = ccb_provider_core::protocol::make_req_id(&job.job_id);
    let reply = format!(
        "{}\nImplementation Receipt\n\nChanged files\n- a.rs\n{}",
        req_id,
        ccb_provider_core::protocol::done_marker(&req_id)
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
        "Implementation Receipt\n\nChanged files\n- a.rs"
    );

    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.confidence, Some(CompletionConfidence::Exact));
    assert_eq!(
        decision.reply,
        "Implementation Receipt\n\nChanged files\n- a.rs"
    );
}

#[test]
fn test_wrap_kimi_prompt_format() {
    let context_path =
        PathBuf::from("/repo/.ccb/agents/slot/provider-state/kimi/home/CCB_KIMI_CONTEXT.md");
    let wrapped = wrap_kimi_prompt("do the thing", "req-12345678", Some(&context_path));
    assert!(wrapped.contains("req-12345678"));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.contains("CCB_KIMI_CONTEXT.md"));
    assert!(wrapped.contains("IMPORTANT:"));
    assert!(wrapped.contains("<<DONE:"));
    assert!(wrapped.ends_with('\n'));
}

#[test]
fn test_kimi_context_path() {
    assert_eq!(
        kimi_context_path(std::path::Path::new("/repo"), "slot1_claude"),
        PathBuf::from(
            "/repo/.ccb/agents/slot1_claude/provider-state/kimi/home/CCB_KIMI_CONTEXT.md"
        )
    );
}

#[test]
fn test_k2_7_input_box_readiness() {
    assert!(looks_like_kimi_input_box_line(
        "│ >                                      K2.7 Code  context: 42%"
    ));
    assert!(!looks_like_kimi_input_box_line("│ > waiting"));
}

#[test]
fn test_non_answer_progress_detection() {
    assert!(looks_like_kimi_non_answer("Run focused tests."));
    assert!(looks_like_kimi_non_answer("Using Read"));
    assert!(looks_like_kimi_non_answer(
        "Let me start by reading the scoped CCB context file"
    ));
    assert!(looks_like_kimi_non_answer(
        "No docs lint script. We can note."
    ));
    assert!(looks_like_kimi_non_answer(
        "User says previous reply was just process fragment"
    ));
    assert!(looks_like_kimi_non_answer(
        "They want final Documentation Receipt only."
    ));
    assert!(!looks_like_kimi_non_answer(
        "Implementation Receipt\n\nChanged files\n- src/a.rs"
    ));
}

#[test]
fn test_clean_kimi_pane_reply_removes_done_and_input_box() {
    let text = "result\n│ >                                      K2.7 Code  context: 42%\n<<DONE:req-12345678>>";
    assert_eq!(clean_kimi_pane_reply(text, "req-12345678"), "result");
}

#[test]
fn test_extract_reply_uses_first_req_id_and_keeps_multiline_receipt() {
    let text = "noise\nreq-12345678\nImplementation Receipt\n\nChanged files\n- a.rs\n\nCCB_REQ_ID: req-12345678\nCommands / results\n- pass\n<<DONE:req-12345678>>";
    assert_eq!(
        extract_reply_for_req(text, "req-12345678"),
        "Implementation Receipt\n\nChanged files\n- a.rs\n\nCCB_REQ_ID: req-12345678\nCommands / results\n- pass"
    );
}

#[test]
fn test_extract_reply_rejects_progress_only_text() {
    let text = "req-12345678\nRun focused tests.\n<<DONE:req-12345678>>";
    assert_eq!(extract_reply_for_req(text, "req-12345678"), "");
}

#[test]
fn test_is_done_text() {
    let text = "some text <<DONE:req-12345678>> more";
    assert!(is_done_text(text));
}

#[test]
fn test_request_and_result_defaults() {
    let req = KimiRequest {
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

    let result = KimiResult {
        exit_code: 0,
        reply: "test reply".into(),
        req_id: "abc123".into(),
        session_key: "kimi:xyz".into(),
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
    let session_path = tmp.path().join(".kimi-session");
    std::fs::write(&session_path, "{}").unwrap();

    let found = find_project_session_file(tmp.path(), None).unwrap();
    assert_eq!(found, session_path);
}

#[test]
fn test_load_project_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    let session_path = tmp.path().join(".kimi-session");
    std::fs::write(
        &session_path,
        r#"{"kimi_session_id":"s1","kimi_context_path":"/repo/context.md"}"#,
    )
    .unwrap();

    let session = load_project_session(tmp.path(), None).unwrap();
    assert_eq!(session.kimi_session_id(), "s1");
    assert_eq!(session.kimi_context_path(), "/repo/context.md");
    assert_eq!(session.session_file, session_path);
}

#[test]
fn test_project_session_default() {
    let session = KimiProjectSession::default();
    assert!(session.kimi_session_id().is_empty());
    assert!(session.kimi_session_path().is_empty());
    assert!(session.kimi_context_path().is_empty());
}
