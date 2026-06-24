use ccbr_completion::models::{CompletionItemKind, CompletionStatus, JobRecord};
use ccbr_providers::execution::ExecutionAdapter;
use ccbr_providers::native_cli_support::{
    clear_tracked_processes, observe_stdout_output, NativeCliExecutionAdapter,
    NativeCliExecutionConfig, OutputKind,
};
use ccbr_providers::providers::crush::build_execution_adapter as build_crush_adapter;

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn prepare_work_dir(tmp: &tempfile::TempDir, session_filename: &str) {
    std::fs::write(
        tmp.path().join(session_filename),
        r#"{"runtime_dir":"/tmp/rt"}"#,
    )
    .unwrap();
}

#[test]
fn test_native_cli_adapter_start_creates_submission() {
    clear_tracked_processes();
    let tmp = tempfile::TempDir::new().unwrap();
    prepare_work_dir(&tmp, ".mock-session");

    let config = NativeCliExecutionConfig::new("mock", |_req| {
        vec!["/bin/echo".to_string(), "hello".to_string()]
    })
    .with_session_filename(".mock-session")
    .with_output_kind(OutputKind::Stdout)
    .with_observer(observe_stdout_output);
    let adapter = NativeCliExecutionAdapter::new(config);

    let job = JobRecord::new("j1", "agent1", "mock")
        .with_workspace_path(tmp.path().to_string_lossy().to_string());
    let submission = adapter.start(&job, None, &fake_now());

    assert_eq!(submission.provider, "mock");
    assert!(!submission.is_terminal());
    assert_eq!(
        submission
            .runtime_state
            .get("mode")
            .unwrap()
            .as_str()
            .unwrap(),
        "mock_run"
    );
    assert!(submission.runtime_state.contains_key("stdout_path"));
    assert!(submission.runtime_state.contains_key("pid"));
}

#[test]
fn test_native_cli_adapter_start_fails_without_session() {
    clear_tracked_processes();
    let tmp = tempfile::TempDir::new().unwrap();

    let config = NativeCliExecutionConfig::new("mock", |_req| {
        vec!["/bin/echo".to_string(), "hello".to_string()]
    })
    .with_session_filename(".missing-session")
    .with_output_kind(OutputKind::Stdout)
    .with_observer(observe_stdout_output);
    let adapter = NativeCliExecutionAdapter::new(config);

    let job = JobRecord::new("j1", "agent1", "mock")
        .with_workspace_path(tmp.path().to_string_lossy().to_string());
    let submission = adapter.start(&job, None, &fake_now());

    assert_eq!(submission.runtime_state.get("mode").unwrap(), "error");
    assert!(submission
        .runtime_state
        .get("error")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("session_file_missing"));
}

#[test]
fn test_native_cli_adapter_poll_stdout_done_marker() {
    clear_tracked_processes();
    let tmp = tempfile::TempDir::new().unwrap();
    prepare_work_dir(&tmp, ".mock-session");

    let config = NativeCliExecutionConfig::new("mock", |_req| {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf 'hello\\n<<DONE:req-12345678>>\\n'".to_string(),
        ]
    })
    .with_session_filename(".mock-session")
    .with_output_kind(OutputKind::Stdout)
    .with_observer(observe_stdout_output);
    let adapter = NativeCliExecutionAdapter::new(config);

    let job = JobRecord::new("j1", "agent1", "mock")
        .with_workspace_path(tmp.path().to_string_lossy().to_string());
    let submission = adapter.start(&job, None, &fake_now());

    // Give the subprocess a moment to finish writing.
    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    assert!(
        result
            .items
            .iter()
            .any(|i| i.kind == CompletionItemKind::AnchorSeen),
        "expected anchor_seen item"
    );

    let decision = result.decision.expect("expected terminal decision");
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "hello");
}

#[test]
fn test_native_cli_adapter_poll_jsonl_final_event() {
    clear_tracked_processes();
    let tmp = tempfile::TempDir::new().unwrap();
    prepare_work_dir(&tmp, ".mock-session");

    let config = NativeCliExecutionConfig::new("mock", |_req| {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf '{\"type\":\"final\",\"text\":\"hello\",\"finish_reason\":\"stop\"}\\n'"
                .to_string(),
        ]
    })
    .with_session_filename(".mock-session")
    .with_output_kind(OutputKind::Jsonl);
    let adapter = NativeCliExecutionAdapter::new(config);

    let job = JobRecord::new("j1", "agent1", "mock")
        .with_workspace_path(tmp.path().to_string_lossy().to_string())
        .with_request_body("do it");
    let submission = adapter.start(&job, None, &fake_now());

    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "hello");
}

#[test]
fn test_native_cli_adapter_poll_detects_nonzero_exit() {
    clear_tracked_processes();
    let tmp = tempfile::TempDir::new().unwrap();
    prepare_work_dir(&tmp, ".mock-session");

    let config = NativeCliExecutionConfig::new("mock", |_req| {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "exit 1".to_string(),
        ]
    })
    .with_session_filename(".mock-session")
    .with_output_kind(OutputKind::Stdout)
    .with_observer(observe_stdout_output);
    let adapter = NativeCliExecutionAdapter::new(config);

    let job = JobRecord::new("j1", "agent1", "mock")
        .with_workspace_path(tmp.path().to_string_lossy().to_string());
    let submission = adapter.start(&job, None, &fake_now());

    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = adapter
        .poll(&submission, &fake_now())
        .expect("expected poll result");
    let decision = result.decision.expect("expected terminal decision");
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert!(decision.reason.as_ref().unwrap().contains("failed"));
}

#[test]
fn test_crush_adapter_provider_name() {
    let adapter = build_crush_adapter();
    assert_eq!(adapter.provider(), "crush");
}

#[test]
fn test_crush_adapter_start_with_mock_command() {
    clear_tracked_processes();
    let tmp = tempfile::TempDir::new().unwrap();

    // Point the crush provider start command to a binary that exists.
    std::env::set_var("CRUSH_START_CMD", "/bin/echo");
    std::fs::write(tmp.path().join(".crush-session"), "{}").unwrap();

    let job = JobRecord::new("j1", "agent1", "crush")
        .with_workspace_path(tmp.path().to_string_lossy().to_string())
        .with_request_body("hello");
    let adapter = build_crush_adapter();
    let submission = adapter.start(&job, None, &fake_now());

    assert_eq!(submission.provider, "crush");
    assert_eq!(
        submission
            .runtime_state
            .get("mode")
            .unwrap()
            .as_str()
            .unwrap(),
        "crush_run"
    );
    assert!(submission.runtime_state.contains_key("stdout_path"));
}
