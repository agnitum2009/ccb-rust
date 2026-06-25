
    use super::*;
    use tempfile::TempDir;

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
