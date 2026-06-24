use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ccbr_completion::models::{
    CompletionConfidence, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::contracts::LaunchMode;
use ccbr_provider_core::manifest::RuntimeMode;
use ccbr_providers::execution::target::{with_prompt_target_override, PromptTarget};
use ccbr_providers::execution::{ExecutionAdapter, ProviderRuntimeContext};
use ccbr_providers::providers::agy::{
    backend, build_runtime_launcher, build_session_binding, extract_reply_for_req,
    find_project_session_file, load_project_session, manifest, pane_contains_req_anchor,
    request_anchor, wrap_agy_prompt, AgyExecutionAdapter, PROVIDER_NAME,
};
use serde_json::Value;

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn later_now(secs: f64) -> String {
    use chrono::TimeZone;
    let base = chrono::Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let dt = base + chrono::Duration::milliseconds((secs * 1000.0) as i64);
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn job_with_body(job_id: &str, agent_name: &str, body: &str) -> JobRecord {
    JobRecord::new(job_id, agent_name, PROVIDER_NAME).with_request_body(body)
}

fn write_agy_session(dir: &std::path::Path, pane_id: &str) -> PathBuf {
    let path = dir.join(".agy-session");
    let data = serde_json::json!({
        "ccbr_session_id": "session-123",
        "pane_id": pane_id,
        "agy_session_id": "session-123",
        "agy_session_path": path.to_string_lossy().to_string(),
        "runtime_dir": dir.to_string_lossy().to_string(),
    });
    std::fs::write(&path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    path
}

#[derive(Clone, Default)]
struct RecordingTarget {
    sent: Arc<Mutex<Vec<(String, String)>>>,
    content: Arc<Mutex<String>>,
}

impl PromptTarget for RecordingTarget {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
        self.sent
            .lock()
            .unwrap()
            .push((pane_id.to_string(), text.to_string()));
        Ok(())
    }

    fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Result<String, String> {
        Ok(self.content.lock().unwrap().clone())
    }
}

fn start_with_mock_target(
    adapter: &AgyExecutionAdapter,
    job: &JobRecord,
    ctx: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ccbr_providers::execution::ProviderSubmission {
    let target: Arc<dyn PromptTarget> = Arc::new(RecordingTarget::default());
    with_prompt_target_override(target, || adapter.start(job, ctx, now))
}

#[test]
fn test_manifest_capabilities_and_profiles() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(m.supports_resume);
    assert!(m.supports_permission_auto);
    assert!(!m.supports_stream_watch);
    assert!(m.supports_subagents);
    assert!(m.supports_workspace_attach);

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
    assert_eq!(binding.session_id_attr, "agy_session_id");
    assert_eq!(binding.session_path_attr, "agy_session_path");
    assert!(b.runtime_launcher.is_some());
    let launcher = b.runtime_launcher.unwrap();
    assert_eq!(launcher.provider, PROVIDER_NAME);
    assert_eq!(launcher.launch_mode, LaunchMode::SimpleTmux);
}

#[test]
fn test_session_binding_builder() {
    let binding = build_session_binding();
    assert_eq!(binding.provider, PROVIDER_NAME);
    assert_eq!(binding.session_id_attr, "agy_session_id");
    assert_eq!(binding.session_path_attr, "agy_session_path");
}

#[test]
fn test_runtime_launcher_builder() {
    let launcher = build_runtime_launcher();
    assert_eq!(launcher.provider, PROVIDER_NAME);
    assert_eq!(launcher.launch_mode, LaunchMode::SimpleTmux);
}

#[test]
fn test_execution_adapter_provider_name() {
    let adapter = AgyExecutionAdapter;
    assert_eq!(adapter.provider(), PROVIDER_NAME);
}

#[test]
fn test_start_without_workspace_returns_error_submission() {
    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j1", "agent1", "hello");
    let submission = adapter.start(&job, None, &fake_now());
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(submission.source_kind, CompletionSourceKind::TerminalText);
    assert!(!submission.is_terminal());
    let error = submission
        .runtime_state
        .get("error")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(error, "work_dir_missing");
}

#[test]
fn test_start_without_session_returns_error_submission() {
    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j2", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/nonexistent_workspace".to_string()),
        ..Default::default()
    };
    let submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());
    let error = submission
        .runtime_state
        .get("error")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(error, "agy_session_file_missing");
}

#[test]
fn test_start_creates_active_submission_from_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j3", "agent1", "hello agy");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    assert_eq!(submission.job_id, "j3");
    assert_eq!(submission.agent_name, "agent1");
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::SessionEventLog
    );
    assert!(!submission.is_terminal());

    let state = &submission.runtime_state;
    assert_eq!(state.get("mode").unwrap(), "native_transcript_log");
    assert_eq!(state.get("pane_id").unwrap(), "%42");
    assert!(state.get("prompt_sent").unwrap().as_bool().unwrap());
    let prompt_text = state.get("prompt_text").unwrap().as_str().unwrap();
    assert!(prompt_text.contains("CCBR_REQ_ID:"));
    assert!(prompt_text.contains("CCBR_DONE:"));
    assert!(prompt_text.contains("hello agy"));
}

#[test]
fn test_load_project_session_parses_fields() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = write_agy_session(tmp.path(), "%7");

    let session = load_project_session(tmp.path(), None).unwrap();
    assert_eq!(session.session_file, path);
    assert_eq!(session.pane_id(), "%7");
    assert_eq!(session.agy_session_id(), "session-123");
    assert!(!session.agy_session_path().is_empty());
}

#[test]
fn test_find_project_session_file_with_instance() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join(".agy-reviewer-session");
    std::fs::write(&path, serde_json::json!({"pane_id": "%9"}).to_string()).unwrap();
    let found = find_project_session_file(tmp.path(), Some("reviewer")).unwrap();
    assert_eq!(found, path);
}

#[test]
fn test_poll_returns_none_without_done_marker() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j4", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());
    assert!(adapter.poll(&submission, &fake_now()).is_none());
}

#[test]
fn test_poll_terminal_completed_on_done_marker() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j5", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    let req_id = request_anchor(&job.job_id);
    let pane_text = format!(
        "CCBR_REQ_ID: {req_id}\n\necho done\nCCBR_DONE: {req_id}\n\nreply body\nCCBR_DONE: {req_id}"
    );
    submission
        .runtime_state
        .insert("pane_content".to_string(), Value::String(pane_text));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    assert!(result.items.is_empty());
    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reason.as_deref().unwrap(), "pane_done_marker");
    assert_eq!(decision.reply, "reply body");
    assert_eq!(decision.confidence, Some(CompletionConfidence::Observed));
}

#[test]
fn test_poll_terminal_incomplete_on_empty_reply() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j6", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    let req_id = request_anchor(&job.job_id);
    let pane_text = format!("CCBR_REQ_ID: {req_id}\nCCBR_DONE: {req_id}\nCCBR_DONE: {req_id}");
    submission
        .runtime_state
        .insert("pane_content".to_string(), Value::String(pane_text));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Incomplete);
    assert_eq!(decision.reason.as_deref().unwrap(), "pane_done_empty_reply");
    assert!(decision
        .diagnostics
        .get("empty_reply")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[test]
fn test_poll_terminal_failed_after_max_wait() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j7", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());
    submission.runtime_state.insert(
        "pane_content".to_string(),
        Value::String("no markers".to_string()),
    );

    let now = later_now(301.0);
    let result = adapter
        .poll(&submission, &now)
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert_eq!(decision.reason.as_deref().unwrap(), "pane_quiet_timeout");
}

#[test]
fn test_poll_no_terminal_without_done_marker() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j8", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    let req_id = request_anchor(&job.job_id);
    let pane_text = format!("CCBR_REQ_ID: {req_id}\n\npartial reply without done marker");
    submission
        .runtime_state
        .insert("pane_content".to_string(), Value::String(pane_text));

    let now = later_now(6.0);
    assert!(adapter.poll(&submission, &now).is_none());
}

#[test]
fn test_poll_no_terminal_with_unchanged_content() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j8b", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    let req_id = request_anchor(&job.job_id);
    let pane_text = format!("CCBR_REQ_ID: {req_id}\n\nstill thinking");
    submission
        .runtime_state
        .insert("pane_content".to_string(), Value::String(pane_text));

    // Even after a long quiet period, if the content has not changed we should
    // not infer completion (matching the Python AGY extraction behaviour).
    let now = later_now(10.0);
    assert!(adapter.poll(&submission, &now).is_none());
}

#[test]
fn test_poll_terminal_incomplete_on_anchor_timeout() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j9", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());
    submission.runtime_state.insert(
        "pane_content".to_string(),
        Value::String("no anchor".to_string()),
    );

    let now = later_now(121.0);
    let result = adapter
        .poll(&submission, &now)
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert_eq!(decision.status, CompletionStatus::Incomplete);
    assert_eq!(
        decision.reason.as_deref().unwrap(),
        "agy_input_unresponsive"
    );
}

#[test]
fn test_poll_terminal_failed_on_send_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j10", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());
    submission.runtime_state.insert(
        "send_error".to_string(),
        Value::String("tmux_not_found".to_string()),
    );

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert_eq!(
        decision.reason.as_deref().unwrap(),
        "send_failed:tmux_not_found"
    );
}

#[test]
fn test_wrap_agy_prompt_format() {
    let req_id = "<<BEGIN:req-12345678>>";
    let wrapped = wrap_agy_prompt("do the thing", req_id);
    assert!(wrapped.contains(&format!("CCBR_REQ_ID: {req_id}")));
    assert!(wrapped.contains(&format!("CCBR_DONE: {req_id}")));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.ends_with('\n'));
}

#[test]
fn test_request_anchor_deterministic() {
    let a = request_anchor("job-123");
    let b = request_anchor("job-123");
    assert_eq!(a, b);
    assert!(a.starts_with("<<BEGIN:"));
    assert!(a.ends_with(">>"));
}

#[test]
fn test_extract_reply_for_req() {
    let req_id = "<<BEGIN:req-12345678>>";
    let text =
        format!("CCBR_REQ_ID: {req_id}\necho\nCCBR_DONE: {req_id}\nhello agy\nCCBR_DONE: {req_id}");
    let (reply, done) = extract_reply_for_req(&text, req_id);
    assert!(done);
    assert_eq!(reply, "hello agy");
}

#[test]
fn test_pane_contains_req_anchor() {
    let req_id = "<<BEGIN:req-12345678>>";
    assert!(pane_contains_req_anchor(
        &format!("prefix CCBR_REQ_ID: {req_id} suffix"),
        req_id
    ));
    assert!(!pane_contains_req_anchor("no anchor", req_id));
}

#[test]
fn test_start_dispatches_prompt_to_pane() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j-dispatch", "agent1", "hello agy");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };

    let target = RecordingTarget::default();
    let sent = target.sent.clone();
    let submission = with_prompt_target_override(Arc::new(target), || {
        adapter.start(&job, Some(&ctx), &fake_now())
    });

    assert!(submission
        .runtime_state
        .get("prompt_sent")
        .and_then(Value::as_bool)
        .unwrap());
    let guard = sent.lock().unwrap();
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].0, "%42");
    assert!(guard[0].1.contains("CCBR_REQ_ID:"));
    assert!(guard[0].1.contains(&request_anchor(&job.job_id)));
}

#[test]
fn test_poll_completes_from_native_transcript() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let brain_root = tmp.path().join("brain");
    let conv = brain_root.join("conv1");
    let logs = conv.join(".system_generated").join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let transcript = logs.join("transcript.jsonl");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j-native", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    let req_id = request_anchor(&job.job_id);
    let prompt = submission
        .runtime_state
        .get("prompt_text")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    assert!(prompt.contains(&req_id));

    // Point AGY transcript discovery at the brain root.
    submission.runtime_state.insert(
        "agy_home".to_string(),
        Value::String(brain_root.to_string_lossy().to_string()),
    );

    let user_event = serde_json::json!({
        "source": "USER",
        "type": "USER_INPUT",
        "content": format!("CCBR_REQ_ID: {req_id}"),
    });
    let model_event = serde_json::json!({
        "source": "MODEL",
        "type": "MODEL_RESPONSE",
        "content": "native reply",
    });
    std::fs::write(
        &transcript,
        format!(
            "{}\n{}\n",
            serde_json::to_string(&user_event).unwrap(),
            serde_json::to_string(&model_event).unwrap()
        ),
    )
    .unwrap();

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(
        decision.reason.as_deref().unwrap(),
        "agy_transcript_response_done"
    );
    assert_eq!(decision.reply, "native reply");
    assert_eq!(decision.confidence, Some(CompletionConfidence::Exact));
}

#[test]
fn test_poll_native_anchor_missing_timeout() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_agy_session(tmp.path(), "%42");

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j-missing", "agent1", "hello");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = start_with_mock_target(&adapter, &job, Some(&ctx), &fake_now());

    let now = later_now(121.0);
    let result = adapter
        .poll(&submission, &now)
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Incomplete);
    assert_eq!(
        decision.reason.as_deref().unwrap(),
        "agy_native_anchor_missing"
    );
}

#[test]
fn test_resume_returns_none() {
    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j11", "agent1", "hello");
    let submission = adapter.start(&job, None, &fake_now());
    let persisted = ccbr_providers::execution::PersistedExecutionState::new(
        submission.clone(),
        None,
        false,
        fake_now(),
    );
    assert!(adapter
        .resume(&job, &submission, None, &persisted, &fake_now())
        .is_none());
}
