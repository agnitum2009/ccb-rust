use super::*;
use ccbr_providers::execution::target::{with_prompt_target_override, PromptTarget};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[derive(Default)]
struct RecordingPromptTarget {
    sent: Arc<Mutex<Vec<(String, String)>>>,
}

impl PromptTarget for RecordingPromptTarget {
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

#[test]
fn test_ccbrd_app_lifecycle() {
    let dir = TempDir::new().unwrap();
    let app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    assert!(!app.is_shutdown_requested());
    app.request_shutdown();
    assert!(app.is_shutdown_requested());
}

#[test]
fn test_startup_shutdown() {
    let dir = TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    app.start().unwrap();
    app.shutdown().unwrap();
}

#[test]
fn test_shutdown_forces_workspace_exit_cleanup() {
    let dir = TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    app.registry.register(AgentRuntimeEntry {
        agent_name: "agent1".into(),
        provider: "codex".into(),
        state: "idle".into(),
        health: "healthy".into(),
        pane_id: Some("%9".into()),
        workspace_path: None,
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });
    app.project_namespace
        .mount(crate::services::project_namespace::ProjectNamespace {
            project_root: dir.path().display().to_string(),
            project_id: app.project_id().to_string(),
            tmux_socket_path: app.tmux_socket_path(),
            tmux_socket_name: "tmux".into(),
            tmux_session_name: app.tmux_session_name(),
            agent_names: vec!["agent1".into()],
            windows: vec![crate::services::project_namespace::NamespaceWindow {
                name: "ccbr".into(),
                window_id: None,
                agents: vec!["agent1".into()],
            }],
            agent_panes: [("agent1".into(), "%9".into())].into_iter().collect(),
            active_panes: vec!["%9".into()],
            namespace_epoch: 1,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .unwrap();

    app.shutdown().expect("shutdown should succeed");

    let report = app
        .last_shutdown_report
        .as_ref()
        .expect("shutdown report should be recorded");
    assert!(
        report.actions_taken.iter().any(|a| a == "forced_cleanup"),
        "user-facing shutdown must fully exit the workspace, not only stop ccbrd/sidebar"
    );
}

#[test]
fn test_submit_heartbeat_delivers_codex_prompt_through_provider() {
    let dir = TempDir::new().unwrap();
    let ccbr_dir = dir.path().join(".ccbr");
    let session_root = dir.path().join("codex-sessions");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::create_dir_all(&session_root).unwrap();
    std::fs::write(
        ccbr_dir.join("ccbr.config"),
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "codex"
target = "agent1"
"#,
    )
    .unwrap();
    std::fs::write(
        ccbr_dir.join(".codex-agent1-session"),
        serde_json::json!({
            "terminal": "tmux",
            "pane_id": "%5",
            "codex_session_root": session_root.to_string_lossy().to_string(),
        })
        .to_string(),
    )
    .unwrap();

    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    let response = app.handle_rpc(
        &serde_json::json!({
            "op": "submit",
            "request": {
                "project_id": app.project_id(),
                "to_agent": "agent1",
                "from_actor": "user",
                "body": "deliver through submit heartbeat"
            }
        })
        .to_string(),
    );
    assert!(response.contains("\"ok\":true"));

    let recorder = RecordingPromptTarget::default();
    let sent = recorder.sent.clone();
    let target: Arc<dyn PromptTarget> = Arc::new(recorder);
    app.supervision
        .store_mut()
        .record_escalation("agent1", "test_without_tmux");
    with_prompt_target_override(target, || app.heartbeat());

    let sent = sent.lock().unwrap();
    assert_eq!(
        sent.len(),
        1,
        "Python-style submit must drive provider-owned prompt delivery on heartbeat"
    );
    assert!(sent[0].0.starts_with('%'));
    assert!(sent[0].1.starts_with("<<BEGIN:req-"));
    assert!(sent[0].1.contains("deliver through submit heartbeat"));
}

#[test]
fn test_registry_registers_non_default_window_agents_for_provider_launch() {
    let dir = TempDir::new().unwrap();
    let ccbr_dir = dir.path().join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        ccbr_dir.join("ccbr.config"),
        r#"version = 2
entry_window = "main"
default_agents = ["main_a"]

[windows]
main = "main_a:codex"
archi = "archi:codex;mother:codex"

[agents.main_a]
provider = "codex"
target = "main_a"

[agents.archi]
provider = "codex"
target = "archi"

[agents.mother]
provider = "codex"
target = "mother"
"#,
    )
    .unwrap();

    let app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );

    assert_eq!(
        app.registry.get("main_a").map(|e| e.provider.as_str()),
        Some("codex")
    );
    assert_eq!(
        app.registry.get("archi").map(|e| e.provider.as_str()),
        Some("codex"),
        "non-default agents referenced by windows must be registered so topology start can launch their provider"
    );
    assert_eq!(
        app.registry.get("mother").map(|e| e.provider.as_str()),
        Some("codex")
    );
}

#[test]
fn test_handle_rpc_ping() {
    let dir = TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    let response = app.handle_rpc(r#"{"op":"ping","request":{"target":"ccbrd"}}"#);
    assert!(response.contains("pong"));
}

#[test]
fn test_handle_rpc_ping_cli_shape() {
    let dir = TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    let response = app.handle_rpc(r#"{"method":"ping","params":{"target":"ccbrd"}}"#);
    assert!(response.contains("pong"));
    assert!(response.contains("\"result\""));
}

#[test]
fn test_loads_project_config_into_registry() {
    let dir = TempDir::new().unwrap();
    let ccbr_dir = dir.path().join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        ccbr_dir.join("ccbr.config"),
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "codex"
target = "agent1"

[windows]
main = "agent1:codex"
"#,
    )
    .unwrap();

    let app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    let entry = app
        .registry
        .get("agent1")
        .expect("agent1 should be registered");
    assert_eq!(entry.provider, "codex");
    assert_eq!(app.dispatcher.agent_names, vec!["agent1"]);
}

#[test]
fn test_respawn_dead_agents_with_checker_triggers_start_flow() {
    let dir = TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    app.registry.register(AgentRuntimeEntry {
        agent_name: "agent1".into(),
        provider: "codex".into(),
        state: "idle".into(),
        health: "healthy".into(),
        pane_id: Some("%42".into()),
        workspace_path: None,
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });

    // All panes considered dead -> should trigger a start flow respawn.
    app.respawn_dead_agents_with_checker(|_pane_id| false)
        .expect("respawn should succeed with stub backend");

    let entry = app.registry.get("agent1").expect("agent1 still registered");
    assert!(
        entry.pane_id.as_deref() != Some("%42"),
        "pane id should be updated after respawn, got {:?}",
        entry.pane_id
    );
}

#[test]
fn test_respawn_dead_agents_with_checker_no_op_when_alive() {
    let dir = TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    app.registry.register(AgentRuntimeEntry {
        agent_name: "agent1".into(),
        provider: "codex".into(),
        state: "idle".into(),
        health: "healthy".into(),
        pane_id: Some("%42".into()),
        workspace_path: None,
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });

    // All panes considered alive -> no start flow, pane id unchanged.
    app.respawn_dead_agents_with_checker(|_pane_id| true)
        .expect("respawn check should succeed");

    let entry = app.registry.get("agent1").expect("agent1 still registered");
    assert_eq!(entry.pane_id.as_deref(), Some("%42"));
}

#[test]
fn test_shutdown_persists_and_start_restores_running_jobs() {
    use crate::models::api_models::common::JobStatus;
    use crate::models::api_models::messages::MessageEnvelope;
    use crate::models::api_models::records::JobRecord;

    let dir = TempDir::new().unwrap();
    let ccbr_dir = dir.path().join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        ccbr_dir.join("ccbr.config"),
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "codex"
target = "agent1"
"#,
    )
    .unwrap();

    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );

    // Inject a running job directly into the dispatcher.
    let job = JobRecord {
        job_id: "job-1".into(),
        submission_id: None,
        agent_name: "agent1".into(),
        provider: "codex".into(),
        request: MessageEnvelope {
            project_id: "proj-1".into(),
            to_agent: "agent1".into(),
            from_actor: "user".into(),
            body: "keep running".into(),
            task_id: None,
            reply_to: None,
            message_type: "ask".into(),
            delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        },
        status: JobStatus::Running,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        workspace_path: None,
        target_kind: crate::models::api_models::common::TargetKind::Agent,
        target_name: "agent1".into(),
    };
    app.dispatcher.job_store.push(job);
    app.dispatcher
        .state
        .rebuild(&app.dispatcher.job_store.clone());

    app.shutdown().expect("shutdown should succeed");

    // Simulate a fresh daemon instance in the same project.
    let mut restarted = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    restarted.start().expect("start should succeed");

    let restored = restarted
        .dispatcher
        .get("job-1")
        .expect("running job should be restored");
    assert_eq!(restored.status, JobStatus::Running);
    assert_eq!(restored.agent_name, "agent1");
}
