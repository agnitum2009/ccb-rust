use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ccb_daemon::app::CcbdApp;
use ccb_daemon::socket_server::SocketServer;
use ccb_daemon::start_flow::service::StartFlowService;
use ccb_daemon::stop_flow::service::StopFlowService;
use serde_json::json;
use tempfile::TempDir;

fn stub_app(dir: &TempDir) -> CcbdApp {
    CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    )
}

fn stub_app_with_config(dir: &TempDir, agents: &[&str]) -> CcbdApp {
    let ccb_dir = dir.path().join(".ccb");
    std::fs::create_dir_all(&ccb_dir).unwrap();
    let default = format!("{:?}", agents);
    let agents_toml = agents
        .iter()
        .map(|a| format!("[agents.{}]\nprovider = \"{}\"\ntarget = \"{}\"\n", a, a, a))
        .collect::<Vec<_>>()
        .join("");
    let windows = agents
        .first()
        .map(|a| format!("main = \"{}:{}\"", a, a))
        .unwrap_or_default();
    let config = format!(
        "version = 2\ndefault_agents = {}\n\n{}\n[windows]\n{}\n",
        default, agents_toml, windows
    );
    std::fs::write(ccb_dir.join("ccb.config"), config).unwrap();
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

#[test]
fn test_ping_handler() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    let resp = call(&mut app, "ping", json!({"target": "ccbd"}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(resp.get("result").is_some());
    assert_eq!(resp["result"]["target"].as_str(), Some("ccbd"));
}

#[test]
fn test_python_shape_ping() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    let raw = r#"{"op":"ping","request":{"target":"ccbd"}}"#;
    let response = app.handle_rpc(raw);
    let resp: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(resp.get("pong").is_some());

    assert_eq!(resp["target"].as_str(), Some("ccbd"));
    assert_eq!(resp["mount_state"].as_str(), Some("running"));
    assert!(resp["health"].is_string());
    assert!(resp["socket_path"].is_string());
    assert!(resp["tmux_socket_path"].is_string());
    assert!(resp["known_agents"].is_array());
    assert!(resp["namespace_tmux_socket_path"].is_string());
    assert!(resp["namespace_tmux_session_name"].is_string());
    assert!(resp["namespace_workspace_window_name"].is_string());
    assert_eq!(resp["namespace_ui_attachable"].as_bool(), Some(false));
    assert!(resp["diagnostics"].is_object());
    assert_eq!(resp["diagnostics"]["pid_alive"].as_bool(), Some(true));
}

#[test]
fn test_ping_all_targets_shape() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude", "gemini"]);
    app.registry.register(ccb_daemon::services::registry::AgentRuntimeEntry {
        agent_name: "claude".into(),
        provider: "claude".into(),
        state: "idle".into(),
        health: "healthy".into(),
        pane_id: Some("%1".into()),
        workspace_path: Some(dir.path().to_string_lossy().to_string()),
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });
    app.registry.register(ccb_daemon::services::registry::AgentRuntimeEntry {
        agent_name: "gemini".into(),
        provider: "gemini".into(),
        state: "idle".into(),
        health: "healthy".into(),
        pane_id: Some("%2".into()),
        workspace_path: Some(dir.path().to_string_lossy().to_string()),
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });

    let all = call(&mut app, "ping", json!({"target": "all"}));
    assert!(all["ok"].as_bool().unwrap_or(false));
    let result = all["result"].as_object().unwrap();
    assert_eq!(result["target"].as_str(), Some("all"));
    assert_eq!(result["ccbd_state"].as_str(), Some("running"));
    let agents = result["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 2);
    for agent in agents {
        assert!(agent["agent_name"].is_string());
        assert!(agent["provider"].is_string());
        assert!(agent["mount_state"].is_string());
        assert!(agent["runtime_state"].is_string());
        assert!(agent["health"].is_string());
        assert!(agent["diagnostics"].is_object());
    }

    let single = call(&mut app, "ping", json!({"target": "claude"}));
    assert!(single["ok"].as_bool().unwrap_or(false));
    let single_result = single["result"].as_object().unwrap();
    assert_eq!(single_result["agent_name"].as_str(), Some("claude"));
    assert_eq!(single_result["target"].as_str(), Some("claude"));
    assert!(single_result["diagnostics"].is_object());
}

#[test]
fn test_start_stop_flow() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    app.start().unwrap();
    let resp = call(
        &mut app,
        "start",
        json!({"agent_names": ["claude", "gemini"]}),
    );
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["agent_results"].as_array().unwrap().len(), 2);

    // Project view should now show the agents.
    let view = call(&mut app, "project_view", json!({}));
    assert!(view.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let agents = view["result"]["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 2);

    // Stop all should clear them.
    let stop = call(&mut app, "stop-all", json!({"force": true}));
    assert!(stop.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(
        stop["result"]["stopped_agents"].as_array().unwrap().len(),
        2
    );

    let view2 = call(&mut app, "project_view", json!({}));
    assert_eq!(view2["result"]["agents"].as_array().unwrap().len(), 0);
}

#[test]
fn test_submit_cancel() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let job_id = submit["result"]["job_id"].as_str().unwrap();

    let queue = call(&mut app, "queue", json!({"target": "claude"}));
    assert!(queue.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let get = call(&mut app, "get", json!({"job_id": job_id}));
    assert!(get.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let cancel = call(&mut app, "cancel", json!({"job_id": job_id}));
    assert!(cancel.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
}

#[test]
fn test_socket_server_round_trip() {
    let dir = TempDir::new().unwrap();
    let app = stub_app(&dir);
    let socket_path = app.socket_path();
    let app = Arc::new(Mutex::new(app));

    let server = SocketServer::new(&socket_path);
    let handle = server.listen(app.clone(), || {}).unwrap();

    // Give the server a moment to bind.
    thread::sleep(Duration::from_millis(50));

    let request = json!({"method": "ping", "params": {"target": "ccbd"}});
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all((serde_json::to_string(&request).unwrap() + "\n").as_bytes())
        .unwrap();

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_shutdown_handler_requests_shutdown() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    assert!(!app.is_shutdown_requested());
    let resp = call(&mut app, "shutdown", json!({}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(app.is_shutdown_requested());
}

#[test]
fn test_heartbeat_updates_generation() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    app.start().unwrap();
    app.heartbeat();
    let health = app.health_monitor.daemon_health();
    assert!(health.generation > 0);
}

#[test]
fn test_queue_returns_actual_per_agent_state() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let queue = call(&mut app, "queue", json!({"target": "claude"}));
    assert!(queue.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let agent = queue["result"]["agent"].as_object().unwrap();
    assert_eq!(agent["queue_depth"].as_u64(), Some(1));
    assert_eq!(agent.get("active_job_id").and_then(|v| v.as_str()), None);
}

#[test]
fn test_inbox_returns_actual_mailbox_events() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let inbox = call(
        &mut app,
        "inbox",
        json!({"agent_name": "claude", "detail": true}),
    );
    assert!(inbox.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(inbox["result"]["item_count"].as_u64(), Some(1));
    let head = inbox["result"]["head"].as_object().unwrap();
    assert_eq!(head["event_type"].as_str(), Some("task_request"));
}

#[test]
fn test_mailbox_head_returns_head_message() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let head = call(&mut app, "mailbox_head", json!({"agent_name": "claude"}));
    assert!(head.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let head_obj = head["result"]["head"].as_object().unwrap();
    assert_eq!(head_obj["event_type"].as_str(), Some("task_request"));
    assert!(head_obj["inbound_event_id"].as_str().is_some());
}

#[test]
fn test_trace_returns_job_history() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let job_id = submit["result"]["job_id"].as_str().unwrap();

    let trace = call(&mut app, "trace", json!({"target": job_id}));
    assert!(trace.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(trace["result"]["resolved_kind"].as_str(), Some("job"));
    assert_eq!(trace["result"]["job_id"].as_str(), Some(job_id));
    let jobs = trace["result"]["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["job_id"].as_str(), Some(job_id));
    assert_eq!(jobs[0]["status"].as_str(), Some("accepted"));
}

#[test]
fn test_watch_returns_activity_lines_for_target() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let job_id = submit["result"]["job_id"].as_str().unwrap();

    let watch = call(
        &mut app,
        "watch",
        json!({"target": "claude", "start_line": 0}),
    );
    assert!(watch.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(watch["result"]["job_id"].as_str(), Some(job_id));
    let lines = watch["result"]["lines"].as_array().unwrap();
    assert!(!lines.is_empty());
    assert!(lines
        .iter()
        .any(|l| l.as_str().unwrap().contains("accepted")));
}

#[test]
fn test_ack_acknowledges_reply_event() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    // Build a reply event directly in the mailbox layer.
    // Use a self-reply so the caller mailbox exists and reply delivery is queued.
    let envelope = ccb_mailbox::models::MessageEnvelope {
        project_id: "proj-1".into(),
        to_agent: "claude".into(),
        from_actor: "claude".into(),
        body: "hello".into(),
        task_id: None,
        reply_to: None,
        message_type: "task_request".into(),
        delivery_scope: ccb_mailbox::models::DeliveryScope::Agent,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    };
    let job = ccb_mailbox::models::JobRecord {
        job_id: "job_test_1".into(),
        submission_id: None,
        agent_name: "claude".into(),
        provider: "claude".into(),
        request: envelope.clone(),
        status: ccb_mailbox::models::JobStatus::Accepted,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
        workspace_path: None,
        target_kind: ccb_mailbox::models::TargetKind::Agent,
        target_name: "claude".into(),
        provider_instance: None,
        provider_options: json!({}),
    };
    let jobs = vec![job.clone()];
    let message_id = app
        .mailbox
        .record_submission(
            &envelope,
            &jobs,
            Some("sub_ack"),
            "2025-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    app.mailbox
        .mark_attempt_started(&job, "2025-01-01T00:00:00Z");
    let mut completed_job = job.clone();
    completed_job.status = ccb_mailbox::models::JobStatus::Completed;
    let decision = ccb_mailbox::facade_recording::CompletionDecision::completed("done");
    app.mailbox.record_terminal(
        &completed_job,
        &decision,
        "2025-01-01T00:00:01Z",
        true,
        true,
    );

    // Verify the reply event is at the head of the inbox.
    let inbox = app.mailbox_control.inbox("claude", Some(true));
    assert_eq!(inbox["item_count"].as_u64(), Some(1));
    let head = inbox["head"].as_object().unwrap();
    assert_eq!(head["event_type"].as_str(), Some("task_reply"));

    // Acknowledge the reply through the RPC handler.
    let event_id = head["inbound_event_id"].as_str().unwrap();
    let ack = call(
        &mut app,
        "ack",
        json!({"agent_name": "claude", "event_id": event_id}),
    );
    assert!(ack.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(
        ack["result"]["acknowledged_event_type"].as_str(),
        Some("task_reply")
    );

    // Inbox should now be empty.
    let inbox_after = app.mailbox_control.inbox("claude", Some(true));
    assert_eq!(inbox_after["item_count"].as_u64(), Some(0));

    // Trace the message to confirm the reply was recorded.
    let trace = call(&mut app, "trace", json!({"target": message_id}));
    assert!(trace.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(trace["result"]["reply_count"].as_u64(), Some(1));
}

#[test]
fn test_cancel_updates_mailbox_state() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_config(&dir, &["claude"]);

    let submit = call(
        &mut app,
        "submit",
        json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        }),
    );
    assert!(submit.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let job_id = submit["result"]["job_id"].as_str().unwrap();

    let cancel = call(&mut app, "cancel", json!({"job_id": job_id}));
    assert!(cancel.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    // The mailbox trace should reflect the terminal/cancelled outcome.
    let trace = call(&mut app, "trace", json!({"target": job_id}));
    assert!(trace.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let job = trace["result"]["job"].as_object().unwrap();
    assert_eq!(job["job_id"].as_str(), Some(job_id));
    assert_eq!(job["status"].as_str(), Some("cancelled"));
    let events = trace["result"]["events"].as_array().unwrap();
    assert!(events
        .iter()
        .any(|e| e["status"].as_str() == Some("consumed")
            || e["status"].as_str() == Some("abandoned")));
}
