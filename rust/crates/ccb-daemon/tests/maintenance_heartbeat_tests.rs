//! Integration tests for the maintenance heartbeat handler.

use ccb_daemon::app::CcbdApp;
use ccb_daemon::models::lifecycle::{
    build_lifecycle, CcbdLifecycleUpdates, CcbdStartupAgentResult, CcbdStartupReport,
    LIFECYCLE_DESIRED_STATE_RUNNING, LIFECYCLE_PHASE_MOUNTED,
};
use ccb_daemon::start_flow::service::StartFlowService;
use ccb_daemon::stop_flow::service::StopFlowService;

#[test]
fn test_maintenance_tick_includes_startup_summary() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );

    // Seed a lifecycle record and startup report so the handler has data to expose.
    let lifecycle = build_lifecycle(
        app.project_id(),
        chrono::Utc::now().to_rfc3339(),
        LIFECYCLE_DESIRED_STATE_RUNNING,
        LIFECYCLE_PHASE_MOUNTED,
        1,
        CcbdLifecycleUpdates {
            startup_stage: Some(Some("mounted".into())),
            ..Default::default()
        },
    );
    let _ = app.lifecycle.save(&lifecycle);

    app.last_startup_report = Some(CcbdStartupReport {
        project_id: app.project_id().to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        trigger: "start_command".into(),
        status: "ok".into(),
        actions_taken: vec!["daemon_started".into()],
        agent_results: vec![CcbdStartupAgentResult {
            agent_name: "demo".into(),
            status: "started".into(),
            reason: None,
            pane_id: Some("%1".into()),
        }],
        failure_reason: None,
        api_version: ccb_daemon::models::api_models::common::API_VERSION,
    });

    let response = app.handle_rpc(r#"{"op":"maintenance_tick","request":{"force":true}}"#);
    let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert!(parsed
        .get("ticked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false));

    let startup_summary = parsed
        .get("startup_summary")
        .expect("startup_summary missing");
    assert_eq!(
        startup_summary
            .get("startup_stage")
            .and_then(|v| v.as_str()),
        Some("mounted")
    );
    assert_eq!(
        startup_summary
            .get("startup_status")
            .and_then(|v| v.as_str()),
        Some("mounted")
    );
    assert_eq!(
        startup_summary.get("trigger").and_then(|v| v.as_str()),
        Some("start_command")
    );
    assert_eq!(
        startup_summary.get("status").and_then(|v| v.as_str()),
        Some("ok")
    );
    let agent_results = startup_summary
        .get("agent_results")
        .and_then(|v| v.as_array())
        .expect("agent_results missing");
    assert_eq!(agent_results.len(), 1);
}
