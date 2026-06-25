//! Integration tests verifying that start_flow forwards RPC parameters into
//! the provider launch context.

use camino::Utf8Path;
use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::provider_launcher::{LaunchContext, LaunchResult, Launcher};
use ccbr_daemon::services::registry::{AgentRegistry, AgentRuntimeEntry};
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct CapturedLaunch {
    provider: String,
    agent_name: String,
    restore: bool,
    auto_permission: bool,
    terminal_size: Option<(u32, u32)>,
    startup_timeout_s: Option<f64>,
    startup_args: Vec<String>,
}

#[derive(Default, Clone)]
struct RecordingLauncher {
    calls: Arc<Mutex<Vec<CapturedLaunch>>>,
}

impl RecordingLauncher {
    fn calls(&self) -> Vec<CapturedLaunch> {
        self.calls.lock().unwrap().clone()
    }
}

impl Launcher for RecordingLauncher {
    fn launch(&self, ctx: &LaunchContext) -> Result<LaunchResult, String> {
        self.calls.lock().unwrap().push(CapturedLaunch {
            provider: ctx.provider.to_string(),
            agent_name: ctx.agent_name.to_string(),
            restore: ctx.restore,
            auto_permission: ctx.auto_permission,
            terminal_size: ctx.terminal_size,
            startup_timeout_s: ctx.startup_timeout_s,
            startup_args: ctx.startup_args.to_vec(),
        });
        Ok(LaunchResult {
            command: "echo recorded".into(),
            session_payload: None,
            session_path: None,
        })
    }
}

fn tmp_root() -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = Utf8Path::from_path(dir.path()).unwrap().to_path_buf();
    (dir, path)
}

fn registry_with(agent_name: &str, provider: &str) -> AgentRegistry {
    let mut registry = AgentRegistry::new();
    registry.register(AgentRuntimeEntry {
        agent_name: agent_name.into(),
        provider: provider.into(),
        state: "registered".into(),
        health: "unknown".into(),
        pane_id: None,
        workspace_path: None,
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });
    registry
}

#[test]
fn test_start_flow_forwards_terminal_size_startup_timeout_and_flags() {
    let (_dir, root) = tmp_root();
    let launcher = RecordingLauncher::default();
    let service = StartFlowService::with_launcher(
        ccbr_daemon::start_flow::service::StartFlowMode::Stub,
        launcher.clone(),
    );
    let agents = vec!["demo".to_string()];
    let registry = registry_with("demo", "codex");
    let startup_args = vec!["--verbose".to_string()];

    let (result, _namespace) = service
        .execute(
            &root,
            "pid",
            "/tmp/tmux.sock",
            "session",
            &agents,
            &registry,
            true,
            true,
            None,
            None,
            Some((233, 61)),
            Some(12.5),
            &startup_args,
        )
        .unwrap();

    assert_eq!(result.status, "ok");
    assert!(result
        .actions_taken
        .contains(&"terminal_size_forwarded".to_string()));
    assert!(result
        .actions_taken
        .contains(&"startup_timeout_forwarded".to_string()));
    assert!(result.actions_taken.contains(&"startup_args:1".to_string()));

    let calls = launcher.calls();
    assert_eq!(calls.len(), 1, "provider launch should be called once");
    let ctx = &calls[0];
    assert_eq!(ctx.agent_name, "demo");
    assert_eq!(ctx.provider, "codex");
    assert!(ctx.restore, "restore CLI flag should be forwarded");
    assert!(
        ctx.auto_permission,
        "auto_permission CLI flag should be forwarded"
    );
    assert_eq!(ctx.terminal_size, Some((233, 61)));
    assert_eq!(ctx.startup_timeout_s, Some(12.5));
    assert_eq!(ctx.startup_args, &["--verbose".to_string()]);
}

#[test]
fn test_start_rpc_handler_parses_and_forwards_launch_params() {
    let (dir, _root) = tmp_root();
    let launcher = RecordingLauncher::default();
    let start_flow = StartFlowService::with_launcher(
        ccbr_daemon::start_flow::service::StartFlowMode::Stub,
        launcher.clone(),
    );
    let mut app = CcbdApp::with_backend(dir.path(), start_flow, StopFlowService::with_stub());
    app.registry.register(AgentRuntimeEntry {
        agent_name: "demo".into(),
        provider: "codex".into(),
        state: "registered".into(),
        health: "unknown".into(),
        pane_id: None,
        workspace_path: None,
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });

    let response = app.handle_rpc(
        r#"{
            "op": "start",
            "request": {
                "agent_names": ["demo"],
                "restore": true,
                "auto_permission": true,
                "terminal_width": 233,
                "terminal_height": 61,
                "startup_timeout_s": 12.5,
                "startup_args": ["--verbose"]
            }
        }"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert!(
        parsed.get("status").and_then(|v| v.as_str()) == Some("ok"),
        "start RPC failed: {}",
        response
    );

    let calls = launcher.calls();
    assert_eq!(calls.len(), 1, "launch should be invoked once");
    let ctx = &calls[0];
    assert!(ctx.restore);
    assert!(ctx.auto_permission);
    assert_eq!(ctx.terminal_size, Some((233, 61)));
    assert_eq!(ctx.startup_timeout_s, Some(12.5));
    assert_eq!(ctx.startup_args, vec!["--verbose".to_string()]);
}
