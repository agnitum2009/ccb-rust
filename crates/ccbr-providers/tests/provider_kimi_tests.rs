use ccbr_completion::models::{
    CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::manifest::RuntimeMode;
use ccbr_provider_core::protocol;
use ccbr_providers::execution::{
    with_prompt_target_override, ExecutionAdapter, PromptTarget, ProviderRuntimeContext,
};
use ccbr_providers::providers::kimi::{
    backend, manifest, wrap_kimi_prompt, KimiExecutionAdapter, PROVIDER_NAME,
};
use std::io::Write;
use std::sync::{Arc, Mutex};

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn write_lines(path: &std::path::Path, lines: &[&str]) {
    let mut file = std::fs::File::create(path).unwrap();
    for line in lines {
        writeln!(file, "{}", line).unwrap();
    }
}

/// Mock prompt target that records sent text and reports pane readiness.
#[derive(Default, Clone)]
struct MockTarget {
    sent: Arc<Mutex<Vec<(String, String)>>>,
    ready: Arc<Mutex<bool>>,
}

impl PromptTarget for MockTarget {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
        self.sent
            .lock()
            .unwrap()
            .push((pane_id.to_string(), text.to_string()));
        Ok(())
    }

    fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Result<String, String> {
        if *self.ready.lock().unwrap() {
            Ok("── input\nagent (\n".to_string())
        } else {
            Ok("not ready".to_string())
        }
    }
}

impl MockTarget {
    fn ready(self) -> Self {
        *self.ready.lock().unwrap() = true;
        self
    }
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
fn test_start_creates_native_turn_log_submission() {
    let tmp = tempfile::TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    std::fs::write(work_dir.join(".kimi-agent1-session"), r#"{"pane_id":"%1"}"#).unwrap();

    let target = Arc::new(MockTarget::default().ready());
    let submission = with_prompt_target_override(target.clone(), || {
        let adapter = KimiExecutionAdapter;
        let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
        let ctx = ProviderRuntimeContext {
            agent_name: "agent1".to_string(),
            workspace_path: Some(work_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        adapter.start(&job, Some(&ctx), &fake_now())
    });

    assert_eq!(submission.job_id, "j1");
    assert_eq!(submission.agent_name, "agent1");
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::SessionEventLog
    );
    assert!(!submission.is_terminal());

    let state = &submission.runtime_state;
    assert_eq!(state.get("mode").unwrap(), "native_turn_log");
    assert_eq!(state.get("pane_id").unwrap(), "%1");
    assert!(state
        .get("pending_prompt")
        .unwrap()
        .as_str()
        .unwrap()
        .contains(protocol::REQ_ID_PREFIX));
    assert!(state.get("prompt_sent").and_then(|v| v.as_bool()).unwrap());
    assert_eq!(state.get("backend_type").unwrap(), "tmux");
}

#[test]
fn test_start_fails_without_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();

    let adapter = KimiExecutionAdapter;
    let job = JobRecord::new("j2", "agent2", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());
    assert!(submission
        .runtime_state
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("kimi_session_file_missing"));
}

#[test]
fn test_poll_returns_none_before_anchor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    std::fs::write(work_dir.join(".kimi-agent3-session"), r#"{"pane_id":"%1"}"#).unwrap();

    let target = Arc::new(MockTarget::default().ready());
    let adapter = KimiExecutionAdapter;
    let job = JobRecord::new("j3", "agent3", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = with_prompt_target_override(target.clone(), || {
        adapter.start(&job, Some(&ctx), &fake_now())
    });
    assert!(with_prompt_target_override(target, || {
        adapter.poll(&submission, &fake_now()).is_none()
    }));
}

#[test]
fn test_poll_emits_terminal_decision_on_completed_turn() {
    let tmp = tempfile::TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    std::fs::write(work_dir.join(".kimi-agent4-session"), r#"{"pane_id":"%1"}"#).unwrap();

    let req_id = protocol::request_anchor_for_job("j4");
    let kimi_home = tmp.path().join(".kimi");
    let sessions_root =
        kimi_home
            .join("sessions")
            .join(ccbr_providers::kimi::native_log::kimi_project_hash(
                &work_dir,
            ));
    std::fs::create_dir_all(&sessions_root).unwrap();
    let wire_path = sessions_root.join("sess1").join("wire.jsonl");
    std::fs::create_dir(wire_path.parent().unwrap()).unwrap();
    write_lines(
        &wire_path,
        &[
            &format!(
                r#"{{"type":"turn.prompt","payload":{{"user_input":[{{"text":"{} {}"}}],"turnId":"turn-1"}}}}"#,
                protocol::REQ_ID_PREFIX,
                req_id
            )
            .replace("\\n\\n", "\n\n"),
            r#"{"type":"ContentPart","payload":{"text":"Implementation Receipt"}}"#,
            r#"{"type":"ContentPart","payload":{"text":"\n\nChanged files\n- a.rs"}}"#,
            r#"{"type":"TurnEnd"}"#,
        ],
    );

    let target = Arc::new(MockTarget::default().ready());
    let adapter = KimiExecutionAdapter;
    let job = JobRecord::new("j4", "agent4", PROVIDER_NAME);
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };

    std::env::set_var("KIMI_HOME", &kimi_home);
    let result = with_prompt_target_override(target.clone(), || {
        let submission = adapter.start(&job, Some(&ctx), &fake_now());
        adapter.poll(&submission, &fake_now())
    });
    std::env::remove_var("KIMI_HOME");

    let result = result.expect("expected poll result");
    assert!(result.decision.is_some());
    let decision = result.decision.unwrap();
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(
        decision.reply,
        "Implementation Receipt\n\n\nChanged files\n- a.rs"
    );

    let assistant_final = result
        .items
        .iter()
        .find(|i| i.kind == CompletionItemKind::AssistantFinal);
    assert!(assistant_final.is_some());
}

#[test]
fn test_wrap_kimi_prompt_format() {
    let wrapped = wrap_kimi_prompt("do the thing", "req-12345678");
    assert!(wrapped.contains("req-12345678"));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.starts_with(protocol::REQ_ID_PREFIX));
}
