use std::fs;

use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::models::api_models::common::DeliveryScope;
use ccbr_daemon::models::api_models::messages::MessageEnvelope;
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
use serde_json::json;
use tempfile::TempDir;

fn stub_app(dir: &TempDir) -> CcbdApp {
    CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    )
}

fn call(app: &mut CcbdApp, method: &str, params: serde_json::Value) -> serde_json::Value {
    let request = json!({"method": method, "params": params});
    let raw = serde_json::to_string(&request).unwrap();
    let response = app.handle_rpc(&raw);
    serde_json::from_str(&response).expect("valid json response")
}

fn register_agent(app: &mut CcbdApp, name: &str, provider: &str, workspace: &std::path::Path) {
    use ccbr_daemon::services::registry::AgentRuntimeEntry;
    app.registry.register(AgentRuntimeEntry {
        agent_name: name.to_string(),
        provider: provider.to_string(),
        state: "idle".to_string(),
        health: "healthy".to_string(),
        pane_id: Some("%1".to_string()),
        workspace_path: Some(workspace.to_string_lossy().to_string()),
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });
}

fn make_envelope(to_agent: &str, body: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "proj".to_string(),
        to_agent: to_agent.to_string(),
        from_actor: "user".to_string(),
        body: body.to_string(),
        task_id: None,
        reply_to: None,
        message_type: "chat".to_string(),
        delivery_scope: DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    }
}

#[test]
fn test_maintenance_tick_returns_tick_summary() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();
    register_agent(&mut app, "claude", "claude", dir.path());

    let resp = call(&mut app, "maintenance_tick", json!({}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result.get("ticked").and_then(|v| v.as_bool()), Some(true));
    let agents = result
        .get("agents")
        .and_then(|v| v.as_array())
        .expect("agents array");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].as_str(), Some("claude"));
}

#[test]
fn test_logs_returns_none_when_session_missing() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();
    register_agent(&mut app, "codex", "codex", dir.path());

    let resp = call(&mut app, "logs", json!({"agent_name": "codex"}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(
        result.get("logs_status").and_then(|v| v.as_str()),
        Some("ok")
    );
    assert_eq!(
        result.get("agent_name").and_then(|v| v.as_str()),
        Some("codex")
    );
    let entries = result
        .get("entries")
        .and_then(|v| v.as_array())
        .expect("entries array");
    assert!(entries.is_empty());
}

#[test]
fn test_logs_returns_tail_lines() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();
    register_agent(&mut app, "codex", "codex", dir.path());

    let session_path = dir.path().join(".ccbr").join(".codex-codex-session");
    fs::create_dir_all(session_path.parent().unwrap()).unwrap();
    let content: String = (1..=20).map(|i| format!("line {}\n", i)).collect();
    fs::write(&session_path, content).unwrap();

    let resp = call(&mut app, "logs", json!({"agent_name": "codex", "tail": 5}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    let entries = result
        .get("entries")
        .and_then(|v| v.as_array())
        .expect("entries array");
    assert_eq!(entries.len(), 1);
    let lines = entries[0]
        .get("lines")
        .and_then(|v| v.as_array())
        .expect("lines array");
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].as_str(), Some("line 16"));
    assert_eq!(lines[4].as_str(), Some("line 20"));
}

#[test]
fn test_cleanup_dry_run_reports_orphans_without_removing() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();
    register_agent(&mut app, "claude", "claude", dir.path());

    // Submit and cancel a job so it becomes a terminal orphan.
    let envelope = make_envelope("claude", "hello");
    let receipt = app.dispatcher.submit(&envelope, "claude", None);
    let job_id = receipt.jobs[0].job_id.clone();
    let _ = app.dispatcher.cancel(&job_id);
    assert_eq!(app.dispatcher.job_store.len(), 1);

    let resp = call(
        &mut app,
        "cleanup",
        json!({"dry_run": true, "agent_name": "claude"}),
    );
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(
        result.get("cleanup_status").and_then(|v| v.as_str()),
        Some("ok")
    );
    assert_eq!(
        result.get("orphaned_jobs_removed").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(result.get("dry_run").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(app.dispatcher.job_store.len(), 1);
}

#[test]
fn test_cleanup_removes_orphans_when_not_dry_run() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();
    register_agent(&mut app, "claude", "claude", dir.path());

    let envelope = make_envelope("claude", "hello");
    let receipt = app.dispatcher.submit(&envelope, "claude", None);
    let job_id = receipt.jobs[0].job_id.clone();
    let _ = app.dispatcher.cancel(&job_id);

    let resp = call(
        &mut app,
        "cleanup",
        json!({"dry_run": false, "agent_name": "claude"}),
    );
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(
        result.get("orphaned_jobs_removed").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(result.get("dry_run").and_then(|v| v.as_bool()), Some(false));
    assert!(app.dispatcher.job_store.is_empty());
}

#[test]
fn test_cleanup_counts_stale_temp_files() {
    use std::fs::File;
    use std::time::{Duration, SystemTime};

    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();

    let tmp_dir = dir.path().join(".ccbr").join("tmp");
    fs::create_dir_all(&tmp_dir).unwrap();
    let stale_file = tmp_dir.join("stale.log");
    fs::write(&stale_file, "old").unwrap();

    // Manually set the modification time to well beyond the stale threshold.
    let past = SystemTime::now() - Duration::from_secs(48 * 60 * 60);
    let file = File::options().write(true).open(&stale_file).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(past))
        .unwrap();

    let resp = call(&mut app, "cleanup", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(
        result.get("stale_files_removed").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert!(stale_file.exists(), "dry-run should not delete files");
}
