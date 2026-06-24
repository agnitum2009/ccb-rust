use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::services::registry::AgentRuntimeEntry;
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

fn register_agent(app: &mut CcbdApp, name: &str) {
    app.registry.register(AgentRuntimeEntry {
        agent_name: name.to_string(),
        provider: "claude".to_string(),
        state: "idle".to_string(),
        health: "healthy".to_string(),
        pane_id: Some("%1".to_string()),
        workspace_path: Some(app.project_root.to_string_lossy().to_string()),
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });
}

#[test]
fn test_fault_list_empty() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    let resp = call(&mut app, "fault_list", json!({}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(resp["result"]["rule_count"].as_u64(), Some(0));
}

#[test]
fn test_fault_arm_and_list() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    register_agent(&mut app, "claude");

    let resp = call(
        &mut app,
        "fault_arm",
        json!({
            "agent_name": "claude",
            "task_id": "task-1",
            "reason": "api_error",
            "count": 2,
            "error_message": "drill",
        }),
    );
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(resp["result"]["agent_name"].as_str(), Some("claude"));
    assert_eq!(resp["result"]["remaining_count"].as_u64(), Some(2));

    let list = call(&mut app, "fault_list", json!({}));
    assert_eq!(list["result"]["rule_count"].as_u64(), Some(1));
}

#[test]
fn test_fault_clear_all() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    register_agent(&mut app, "claude");

    call(
        &mut app,
        "fault_arm",
        json!({
            "agent_name": "claude",
            "task_id": "task-1",
            "reason": "transport_error",
            "count": 1,
        }),
    );

    let clear = call(&mut app, "fault_clear", json!({"target": "all"}));
    assert!(clear.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(clear["result"]["cleared_count"].as_u64(), Some(1));

    let list = call(&mut app, "fault_list", json!({}));
    assert_eq!(list["result"]["rule_count"].as_u64(), Some(0));
}

#[test]
fn test_fault_arm_rejects_unknown_agent() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    let resp = call(
        &mut app,
        "fault_arm",
        json!({
            "agent_name": "unknown",
            "task_id": "task-1",
        }),
    );
    assert!(!resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
}
