use std::path::{Path, PathBuf};

use ccb_completion::models::{CompletionItemKind, CompletionStatus, JobRecord};
use ccb_providers::execution::{ExecutionAdapter, ProviderRuntimeContext};
use ccb_providers::opencode::OpenCodeLogReader;
use ccb_providers::providers::opencode::{
    backend, build_runtime_launcher, build_session_binding, find_project_session_file,
    load_project_session, manifest, OpenCodeExecutionAdapter, PROVIDER_NAME,
};
use serde_json::Value;
use tempfile::TempDir;

fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
    path
}

fn make_job_with_request(body: &str) -> JobRecord {
    JobRecord::new("j1", "agent1", PROVIDER_NAME).with_request_body(body)
}

fn setup_storage(work_dir: &Path) -> PathBuf {
    let storage_root = work_dir.parent().unwrap().join("storage");
    std::fs::create_dir_all(storage_root.join("session").join("proj1")).unwrap();
    std::fs::create_dir_all(storage_root.join("message")).unwrap();
    std::fs::create_dir_all(storage_root.join("part").join("m2")).unwrap();

    write_json(
        &storage_root.join("session").join("proj1"),
        "ses_1.json",
        serde_json::json!({
            "id": "session-1",
            "directory": work_dir.to_string_lossy().to_string(),
            "time": {"updated": 1},
        }),
    );
    write_json(
        &storage_root.join("message"),
        "msg_m1.json",
        serde_json::json!({
            "id": "m1",
            "sessionID": "session-1",
            "role": "user",
            "parentID": "m0",
            "time": {"created": 1},
        }),
    );
    write_json(
        &storage_root.join("message"),
        "msg_m2.json",
        serde_json::json!({
            "id": "m2",
            "sessionID": "session-1",
            "role": "assistant",
            "parentID": "m1",
            "time": {"created": 2, "completed": 12345},
        }),
    );
    write_json(
        &storage_root.join("part").join("m2"),
        "prt_p1.json",
        serde_json::json!({
            "id": "p1",
            "messageID": "m2",
            "type": "text",
            "text": "hello world",
            "time": {"start": 2},
        }),
    );
    storage_root
}

fn write_session_file(work_dir: &Path) {
    write_json(
        work_dir,
        ".opencode-agent1-session",
        serde_json::json!({
            "opencode_session_id": "session-1",
            "opencode_project_id": "proj1",
            "work_dir": work_dir.to_string_lossy().to_string(),
            "pane_id": "%1",
        }),
    );
}

#[test]
fn test_manifest() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(!m.supports_resume);
}

#[test]
fn test_backend_has_binding_and_launcher() {
    let b = backend();
    assert_eq!(b.provider(), PROVIDER_NAME);
    assert!(b.session_binding.is_some());
    assert!(b.runtime_launcher.is_some());
}

#[test]
fn test_session_binding_fields() {
    let binding = build_session_binding();
    assert_eq!(binding.provider, PROVIDER_NAME);
    assert_eq!(binding.session_id_attr, "opencode_session_id");
    assert_eq!(binding.session_path_attr, "session_file");
}

#[test]
fn test_load_project_session() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_json(
        &work_dir,
        ".opencode-session",
        serde_json::json!({
            "opencode_session_id": "session-1",
            "opencode_project_id": "proj1",
            "work_dir": work_dir.to_string_lossy().to_string(),
            "pane_id": "%1",
        }),
    );
    let session = load_project_session(&work_dir, None).unwrap();
    assert_eq!(session.opencode_session_id(), Some("session-1"));
    assert_eq!(session.opencode_project_id(), Some("proj1"));
    assert_eq!(session.pane_id(), Some("%1"));
}

#[test]
fn test_find_project_session_file_for_instance() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_json(
        &work_dir,
        ".opencode-reviewer-session",
        serde_json::json!({"opencode_session_id": "session-reviewer"}),
    );
    let path = find_project_session_file(&work_dir, Some("reviewer")).unwrap();
    assert!(path.to_string_lossy().contains("reviewer"));
}

#[test]
fn test_build_runtime_launcher() {
    let launcher = build_runtime_launcher();
    assert_eq!(launcher.provider, PROVIDER_NAME);
}

#[test]
fn test_launch_context_and_payload() {
    use ccb_providers::opencode::{build_session_payload, build_start_cmd, prepare_launch_context};
    let tmp = TempDir::new().unwrap();
    let runtime_dir = tmp.path().join("runtime");
    let ctx = prepare_launch_context(
        Path::new("/project"),
        "agent1",
        Path::new("/workspace"),
        Path::new("/events"),
        &runtime_dir,
    );
    assert!(ctx.opencode_config_path.contains("opencode.json"));

    let start_cmd = build_start_cmd(false, &[], None);
    let payload = build_session_payload(
        &ctx,
        &runtime_dir,
        Path::new("/run"),
        "%1",
        "CCB-agent1-proj",
        &start_cmd,
        "launch-1",
    );
    assert_eq!(payload.get("agent_name").unwrap(), "agent1");
    assert_eq!(payload.get("pane_id").unwrap(), "%1");
    assert_eq!(payload.get("ccb_session_id").unwrap(), "launch-1");
    assert_eq!(
        payload.get("start_cmd").unwrap(),
        &serde_json::Value::String(start_cmd)
    );
}

#[test]
fn test_adapter_start_without_session_is_error() {
    let adapter = OpenCodeExecutionAdapter;
    let job = make_job_with_request("hi");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some("/nonexistent".to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(submission.runtime_state.get("mode").unwrap(), "error");
}

#[test]
fn test_adapter_start_active() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(&work_dir);
    let _storage_root = setup_storage(&work_dir);

    let adapter = OpenCodeExecutionAdapter;
    let job = make_job_with_request("hi");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    assert_eq!(submission.runtime_state.get("mode").unwrap(), "active");
    assert!(submission.runtime_state.contains_key("prompt"));
    assert!(submission.runtime_state.contains_key("request_anchor"));
    assert_eq!(
        submission.source_kind,
        ccb_completion::models::CompletionSourceKind::SessionSnapshot
    );
}

#[test]
fn test_adapter_poll_emits_reply_and_decision() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(&work_dir);
    let _storage_root = setup_storage(&work_dir);

    let adapter = OpenCodeExecutionAdapter;
    let job = make_job_with_request("hi");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    let result = adapter.poll(&submission, "2025-01-01T00:00:01Z").unwrap();
    assert!(!result.items.is_empty());
    assert!(result
        .items
        .iter()
        .any(|i| i.kind == CompletionItemKind::AnchorSeen));
    assert!(result
        .items
        .iter()
        .any(|i| i.kind == CompletionItemKind::AssistantFinal));
    assert!(result
        .items
        .iter()
        .any(|i| i.kind == CompletionItemKind::TurnBoundary));
    assert!(result.decision.is_some());
    assert_eq!(
        result.decision.as_ref().unwrap().status,
        CompletionStatus::Completed
    );
    assert_eq!(result.submission.reply, "hello world");
}

#[test]
fn test_adapter_poll_no_change_returns_none() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(&work_dir);
    // No storage messages => no reply.

    let adapter = OpenCodeExecutionAdapter;
    let job = make_job_with_request("hi");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    let result = adapter.poll(&submission, "2025-01-01T00:00:01Z");
    assert!(result.is_none());
}

#[test]
fn test_reader_uses_storage_root() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    let storage_root = setup_storage(&work_dir);

    let reader = OpenCodeLogReader::new(Some(&storage_root), &work_dir, "proj1", None);
    let state = reader.capture_state();
    assert_eq!(state.get("session_id").unwrap(), "session-1");
}
