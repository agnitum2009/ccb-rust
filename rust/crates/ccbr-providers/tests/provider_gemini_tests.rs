use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ccbr_completion::models::{
    CompletionConfidence, CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::manifest::RuntimeMode;
use ccbr_providers::execution::target::{with_prompt_target_override, PromptTarget};
use ccbr_providers::execution::{ExecutionAdapter, ProviderRuntimeContext};
use ccbr_providers::providers::gemini::{
    backend, current_gemini_home_root, current_gemini_tmp_root, extract_reply_for_req,
    gemini_layout_for_home, gemini_layout_from_session_data, is_done_text, make_req_id, manifest,
    request_anchor, strip_done_text, wrap_gemini_prompt, wrap_gemini_turn_prompt,
    GeminiExecutionAdapter, GeminiHomeLayout, PROVIDER_NAME,
};
use serde_json::Value;

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

#[test]
fn test_manifest_capabilities_and_profiles() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(m.supports_resume);
    assert!(m.supports_permission_auto);
    assert!(m.supports_stream_watch);
    assert!(!m.supports_subagents);
    assert!(m.supports_workspace_attach);

    assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
    assert!(m.supports_runtime_mode(&RuntimeMode::Headless));

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
    assert_eq!(binding.session_id_attr, "gemini_session_id");
    assert_eq!(binding.session_path_attr, "gemini_session_path");
    assert!(b.runtime_launcher.is_some());
}

#[test]
fn test_execution_adapter_provider_name() {
    let adapter = GeminiExecutionAdapter;
    assert_eq!(adapter.provider(), PROVIDER_NAME);
}

#[test]
fn test_start_creates_active_submission() {
    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
    let submission = adapter.start(&job, None, &fake_now());

    assert_eq!(submission.job_id, "j1");
    assert_eq!(submission.agent_name, "agent1");
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::SessionSnapshot
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
    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j2", "agent2", PROVIDER_NAME);
    let submission = adapter.start(&job, None, &fake_now());
    assert!(adapter.poll(&submission, &fake_now()).is_none());
}

#[test]
fn test_poll_emits_terminal_decision_on_done_marker() {
    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j3", "agent3", PROVIDER_NAME);
    let mut submission = adapter.start(&job, None, &fake_now());

    let req_id = make_req_id(&job.job_id);
    let reply = format!(
        "{}\nhello gemini\n{}{}>>",
        request_anchor(&job.job_id),
        ccbr_provider_core::protocol::DONE_PREFIX,
        req_id
    );
    submission
        .runtime_state
        .insert("reply_buffer".to_string(), Value::String(reply));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, CompletionItemKind::SessionSnapshot);
    assert_eq!(
        result.items[0]
            .payload
            .get("reply")
            .unwrap()
            .as_str()
            .unwrap(),
        "hello gemini"
    );

    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.confidence, Some(CompletionConfidence::Exact));
    assert_eq!(decision.reply, "hello gemini");
}

#[test]
fn test_wrap_gemini_prompt_format() {
    let wrapped = wrap_gemini_prompt("do the thing", "req-12345678");
    assert!(wrapped.contains("req-12345678"));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.contains("<<DONE:"));
    assert!(wrapped.ends_with('\n'));
}

#[test]
fn test_wrap_gemini_turn_prompt_format() {
    let wrapped = wrap_gemini_turn_prompt("do the thing", "req-12345678");
    assert!(wrapped.starts_with("req-12345678"));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.ends_with('\n'));
}

#[test]
fn test_extract_reply_for_req() {
    let text = "<<BEGIN:req-12345678>>\nhello gemini\n<<DONE:req-12345678>>";
    assert_eq!(extract_reply_for_req(text, "req-12345678"), "hello gemini");
}

#[test]
fn test_is_done_text_and_strip_done_text() {
    let text = "some reply\n<<DONE:req-12345678>>";
    assert!(is_done_text(text));
    let stripped = strip_done_text(text);
    assert!(!stripped.contains("<<DONE"));
    assert_eq!(stripped.trim(), "some reply");
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
fn test_home_layout_for_home() {
    let layout = gemini_layout_for_home("/home/user");
    assert_eq!(
        layout,
        GeminiHomeLayout {
            home_root: PathBuf::from("/home/user"),
            gemini_dir: PathBuf::from("/home/user/.gemini"),
            settings_path: PathBuf::from("/home/user/.gemini/settings.json"),
            trusted_folders_path: PathBuf::from("/home/user/.gemini/trustedFolders.json"),
            tmp_root: PathBuf::from("/home/user/.gemini/tmp"),
        }
    );
}

#[test]
fn test_gemini_layout_from_session_data() {
    let mut data = HashMap::new();
    data.insert(
        "gemini_home".to_string(),
        Value::String("/tmp/managed".to_string()),
    );
    let layout = gemini_layout_from_session_data(Some(&data)).unwrap();
    assert_eq!(layout.home_root, PathBuf::from("/tmp/managed"));
}

#[test]
fn test_current_gemini_home_root_respects_env() {
    let tmp = std::env::temp_dir();
    let gemini_root = tmp.join("gemini_env_root");
    let gemini_tmp = gemini_root.join(".gemini").join("tmp");
    std::fs::create_dir_all(&gemini_tmp).unwrap();

    std::env::set_var("GEMINI_ROOT", &gemini_tmp);
    let root = current_gemini_home_root();
    std::env::remove_var("GEMINI_ROOT");

    assert_eq!(root, gemini_root);
}

#[test]
fn test_current_gemini_tmp_root_respects_env() {
    let tmp = std::env::temp_dir();
    let gemini_tmp = tmp.join("gemini_env_tmp");
    std::fs::create_dir_all(&gemini_tmp).unwrap();

    std::env::set_var("GEMINI_ROOT", &gemini_tmp);
    let root = current_gemini_tmp_root();
    std::env::remove_var("GEMINI_ROOT");

    assert_eq!(root, gemini_tmp);
}

#[test]
fn test_export_runtime_state_returns_active_mode() {
    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j4", "agent4", PROVIDER_NAME);
    let submission = adapter.start(&job, None, &fake_now());
    let exported = adapter.export_runtime_state(&submission).unwrap();
    assert_eq!(exported.get("mode").unwrap(), "active");
    assert!(exported.contains_key("request_anchor"));
}

#[test]
fn test_start_with_runtime_context_is_ignored() {
    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j5", "agent5", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        agent_name: "agent5".to_string(),
        workspace_path: Some("/tmp/ws".to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    assert_eq!(submission.provider, PROVIDER_NAME);
}

#[test]
fn test_poll_reads_native_session_and_completes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join(".gemini").join("tmp");
    let chats = root.join("myproject").join("chats");
    std::fs::create_dir_all(&chats).unwrap();
    let session_file = chats.join("session-1.json");

    // Start with an initial Gemini message so the reader captures a baseline.
    std::fs::write(
        &session_file,
        serde_json::json!({
            "messages": [{"type": "gemini", "id": "g-1", "content": "typing..."}]
        })
        .to_string(),
    )
    .unwrap();

    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j-native", "agent1", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        session_ref: Some(session_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::SessionSnapshot
    );
    assert!(submission
        .runtime_state
        .get("session_path")
        .and_then(Value::as_str)
        .is_some());

    // Now the model replies with a done marker for this job.
    let req_id = make_req_id(&job.job_id);
    let reply = format!("hello gemini\n<<DONE:{}>>", req_id);
    std::fs::write(
        &session_file,
        serde_json::json!({
            "messages": [
                {"type": "gemini", "id": "g-1", "content": "typing..."},
                {"type": "gemini", "id": "g-2", "content": reply}
            ]
        })
        .to_string(),
    )
    .unwrap();

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    assert_eq!(result.items.len(), 2); // AnchorSeen + SessionSnapshot
    assert_eq!(result.items[0].kind, CompletionItemKind::AnchorSeen);
    assert_eq!(result.items[1].kind, CompletionItemKind::SessionSnapshot);

    let decision = result.decision.expect("expected terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.confidence, Some(CompletionConfidence::Exact));
    assert_eq!(decision.reply, "hello gemini");
}

#[derive(Clone, Default)]
struct RecordingTarget {
    sent: Arc<Mutex<Vec<(String, String)>>>,
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
        Ok(String::new())
    }
}

fn write_gemini_session(dir: &std::path::Path, pane_id: &str) -> PathBuf {
    let path = dir.join(".gemini-session");
    let data = serde_json::json!({
        "gemini_session_id": "session-123",
        "gemini_session_path": path.to_string_lossy().to_string(),
        "pane_id": pane_id,
    });
    std::fs::write(&path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    path
}

#[test]
fn test_poll_dispatches_prompt_to_pane() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_gemini_session(tmp.path(), "%42");

    let adapter = GeminiExecutionAdapter;
    let job = JobRecord::new("j-dispatch", "agent1", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };

    let target = RecordingTarget::default();
    let sent = target.sent.clone();
    let (_submission, result) = with_prompt_target_override(Arc::new(target), || {
        let submission = adapter.start(&job, Some(&ctx), &fake_now());
        assert!(!submission
            .runtime_state
            .get("prompt_sent")
            .and_then(Value::as_bool)
            .unwrap());
        let result = adapter
            .poll(&submission, &fake_now())
            .expect("expected dispatch result");
        (submission, result)
    });

    assert!(result.decision.is_none());
    assert!(result
        .submission
        .runtime_state
        .get("prompt_sent")
        .and_then(Value::as_bool)
        .unwrap());

    let guard = sent.lock().unwrap();
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].0, "%42");
    assert!(guard[0].1.contains("<<DONE:"));
}
