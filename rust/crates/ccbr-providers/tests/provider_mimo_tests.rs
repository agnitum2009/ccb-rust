use std::path::{Path, PathBuf};

use ccbr_completion::models::JobRecord;
use ccbr_providers::execution::{ExecutionAdapter, ProviderRuntimeContext};
use ccbr_providers::mimo::{
    build_runtime_launcher, build_session_binding, build_session_payload, build_start_cmd,
    find_project_session_file, load_project_session, prepare_launch_context, wrap_mimo_prompt,
};
use ccbr_providers::providers::mimo::{backend, manifest, MimoExecutionAdapter, PROVIDER_NAME};
use serde_json::Value;
use tempfile::TempDir;

fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
    path
}

fn write_session_file(work_dir: &Path) {
    write_session_file_for_instance(work_dir, None);
}

fn write_session_file_for_instance(work_dir: &Path, instance: Option<&str>) {
    let filename = match instance {
        Some(name) => format!(".mimo-{}-session", name),
        None => ".mimo-session".to_string(),
    };
    write_json(
        work_dir,
        &filename,
        serde_json::json!({
            "ccbr_session_id": "session-1",
            "mimo_session_id": "session-1",
            "ccbr_project_id": "proj1",
            "workspace_path": work_dir.to_string_lossy().to_string(),
            "work_dir": work_dir.to_string_lossy().to_string(),
            "pane_id": "%1",
            "runtime_dir": work_dir.join(".ccbr").join("runtime").join("mimo").to_string_lossy().to_string(),
            "completion_artifact_dir": work_dir.join(".ccbr").join("runtime").join("mimo").join("completion").to_string_lossy().to_string(),
            "mimo_home": work_dir.join("mimo_home").to_string_lossy().to_string(),
            "mimo_storage_root": work_dir.join("mimo_home").join("data").join("storage").to_string_lossy().to_string(),
            "mimo_config_path": work_dir.join("mimo_home").join("mimocode.json").to_string_lossy().to_string(),
            "agent_events_path": work_dir.join("events.jsonl").to_string_lossy().to_string(),
            "start_cmd": "mimo",
        }),
    );
}

fn make_job(body: &str) -> JobRecord {
    JobRecord::new("j1", "agent1", PROVIDER_NAME).with_request_body(body)
}

#[test]
fn test_manifest() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(m.supports_subagents);
    assert!(m.supports_workspace_attach);
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
    assert_eq!(binding.session_id_attr, "mimo_session_id");
    assert_eq!(binding.session_path_attr, "mimo_session_path");
}

#[test]
fn test_load_project_session() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(&work_dir);
    let session = load_project_session(&work_dir, None).unwrap();
    assert_eq!(session.mimo_session_id(), "session-1");
    assert_eq!(session.pane_id(), Some("%1"));
}

#[test]
fn test_find_project_session_file_for_instance() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_json(
        &work_dir,
        ".mimo-reviewer-session",
        serde_json::json!({"mimo_session_id": "session-reviewer"}),
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
    let tmp = TempDir::new().unwrap();
    let runtime_dir = tmp.path().join("runtime");
    let ctx = prepare_launch_context(
        Path::new("/project"),
        "agent1",
        Path::new("/workspace"),
        Path::new("/events"),
        &runtime_dir,
    );
    assert!(ctx.mimo_config_path.contains("mimocode.json"));

    let start_cmd = build_start_cmd(false, &[], &ctx, None);
    let payload = build_session_payload(
        &ctx,
        &runtime_dir,
        Path::new("/run"),
        "%1",
        "CCBR-agent1-proj",
        &start_cmd,
        "launch-1",
        Path::new("/workspace/.mimo-session"),
    );
    assert_eq!(payload.get("agent_name").unwrap(), "agent1");
    assert_eq!(payload.get("pane_id").unwrap(), "%1");
    assert_eq!(payload.get("ccbr_session_id").unwrap(), "launch-1");
    assert_eq!(payload.get("mimo_session_id").unwrap(), "launch-1");
    assert_eq!(payload.get("start_cmd").unwrap(), &Value::String(start_cmd));
}

#[test]
fn test_wrap_mimo_prompt_format() {
    let wrapped = wrap_mimo_prompt("hello", "ANCHOR-123");
    assert!(wrapped.contains("ANCHOR-123"));
    assert!(wrapped.contains("hello"));
}

#[test]
fn test_adapter_start_without_session_is_error() {
    let adapter = MimoExecutionAdapter;
    let job = make_job("hi");
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
fn test_adapter_start_with_session_creates_mimo_run_submission() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file_for_instance(&work_dir, Some("agent1"));
    // Use `true` as a no-op stand-in so spawn succeeds in test environments.
    std::env::set_var("MIMO_START_CMD", "/bin/true");

    let adapter = MimoExecutionAdapter;
    let job = make_job("hi");
    let ctx = ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    if submission.runtime_state.get("mode").unwrap() != "mimo_run" {
        panic!(
            "expected mimo_run mode, got error: {}",
            submission
                .runtime_state
                .get("error")
                .unwrap_or(&Value::Null)
        );
    }
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert!(submission.runtime_state.contains_key("request_anchor"));
    assert!(submission.runtime_state.contains_key("pid"));
}
