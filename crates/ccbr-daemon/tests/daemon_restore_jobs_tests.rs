use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::models::api_models::common::{DeliveryScope, JobStatus, TargetKind};
use ccbr_daemon::models::api_models::messages::MessageEnvelope;
use ccbr_daemon::models::api_models::records::JobRecord;
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
use tempfile::TempDir;

fn make_test_project(dir: &TempDir) -> CcbdApp {
    let ccbr_dir = dir.path().join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        ccbr_dir.join("ccbr.config"),
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "fake"
target = "agent1"
"#,
    )
    .unwrap();

    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );

    // Include test-double providers in the completion catalog so the fake
    // adapter can be used in these integration tests.
    app.completion_tracker = ccbr_completion::CompletionTrackerService::new(
        app.current_config.clone().unwrap_or_default(),
        ccbr_provider_core::catalog::build_default_provider_catalog(false, true),
        ccbr_completion::CompletionRegistry,
    );

    app
}

fn make_running_job(job_id: &str, agent_name: &str, body: &str) -> JobRecord {
    JobRecord {
        job_id: job_id.into(),
        submission_id: None,
        agent_name: agent_name.into(),
        provider: "fake".into(),
        request: MessageEnvelope {
            project_id: "proj-1".into(),
            to_agent: agent_name.into(),
            from_actor: "user".into(),
            body: body.into(),
            task_id: None,
            reply_to: None,
            message_type: "ask".into(),
            delivery_scope: DeliveryScope::Single,
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
        target_kind: TargetKind::Agent,
        target_name: agent_name.into(),
    }
}

#[test]
fn daemon_restart_restores_running_job_into_dispatcher_and_execution() {
    let dir = TempDir::new().unwrap();
    let mut app = make_test_project(&dir);

    let job = make_running_job("job-restore-1", "agent1", "keep running");
    let completion_job = ccbr_daemon::adapters::completion::to_completion_job_record(&job);
    let ctx = ccbr_providers::execution::ProviderRuntimeContext {
        agent_name: job.agent_name.clone(),
        workspace_path: Some(app.project_root.to_string_lossy().to_string()),
        backend_type: Some("tmux".into()),
        runtime_ref: Some("%1".into()),
        ..Default::default()
    };

    // Seed the execution state store so the job can be resumed after restart.
    let _ = app.execution.start(&completion_job, Some(&ctx));
    app.dispatcher.job_store.push(job);
    app.dispatcher
        .state
        .rebuild(&app.dispatcher.job_store.clone());

    app.shutdown().expect("shutdown should persist state");

    // Simulate a fresh daemon instance in the same project.
    let mut restarted = make_test_project(&dir);
    restarted.start().expect("start should restore state");

    let restored = restarted
        .dispatcher
        .get("job-restore-1")
        .expect("running job should be restored into dispatcher");
    assert_eq!(restored.status, JobStatus::Running);
    assert_eq!(restored.agent_name, "agent1");

    let contexts = restarted.execution.active_contexts();
    assert!(
        contexts.iter().any(|(id, _)| id == "job-restore-1"),
        "restored job should be re-registered with execution service"
    );

    let tracker = restarted.completion_tracker.current("job-restore-1");
    assert!(
        tracker.is_some(),
        "restored job should have a completion tracker"
    );
}

#[test]
fn heartbeat_poll_drives_restored_job_to_completion_without_resubmission() {
    let dir = TempDir::new().unwrap();
    let mut app = make_test_project(&dir);

    let job = make_running_job("job-restore-2", "agent1", "complete me");
    let completion_job = ccbr_daemon::adapters::completion::to_completion_job_record(&job);
    let ctx = ccbr_providers::execution::ProviderRuntimeContext {
        agent_name: job.agent_name.clone(),
        workspace_path: Some(app.project_root.to_string_lossy().to_string()),
        backend_type: Some("tmux".into()),
        runtime_ref: Some("%1".into()),
        ..Default::default()
    };

    let _ = app.execution.start(&completion_job, Some(&ctx));
    app.dispatcher.job_store.push(job);
    app.dispatcher
        .state
        .rebuild(&app.dispatcher.job_store.clone());

    app.shutdown().unwrap();

    let mut restarted = make_test_project(&dir);
    restarted.start().unwrap();

    // The fake provider completes after one poll by default.
    restarted.heartbeat();

    let final_job = restarted
        .dispatcher
        .get("job-restore-2")
        .expect("job should still exist");
    assert!(
        final_job.status.is_terminal(),
        "heartbeat poll should drive restored job to a terminal status, got {:?}",
        final_job.status
    );

    // No new job was submitted; the same job id reached completion.
    let agent_jobs: Vec<_> = restarted
        .dispatcher
        .job_store
        .iter()
        .filter(|j| j.agent_name == "agent1")
        .collect();
    assert_eq!(
        agent_jobs.len(),
        1,
        "restored job should complete without creating a new submission"
    );

    let contexts = restarted.execution.active_contexts();
    assert!(
        !contexts.iter().any(|(id, _)| id == "job-restore-2"),
        "terminal job should be removed from active execution contexts"
    );
}
