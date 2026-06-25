use ccbr_daemon::app::CcbdApp;
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

fn write_config(dir: &TempDir, content: &str) {
    let ccbr_dir = dir.path().join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(ccbr_dir.join("ccbr.config"), content).unwrap();
}

fn call(app: &mut CcbdApp, method: &str, params: serde_json::Value) -> serde_json::Value {
    let request = json!({"method": method, "params": params});
    let raw = serde_json::to_string(&request).unwrap();
    let response = app.handle_rpc(&raw);
    serde_json::from_str(&response).expect("valid json response")
}

fn base_config() -> String {
    r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "codex"
target = "agent1"

[windows]
main = "agent1:codex"
"#
    .to_string()
}

fn mount_stub_namespace(app: &mut CcbdApp) {
    app.project_namespace
        .mount(ccbr_daemon::services::project_namespace::ProjectNamespace {
            project_root: app.project_root.to_string_lossy().to_string(),
            project_id: app.project_id().to_string(),
            tmux_socket_path: app.tmux_socket_path(),
            tmux_socket_name: "default".into(),
            tmux_session_name: app.tmux_session_name(),
            agent_names: vec!["agent1".into()],
            windows: vec![ccbr_daemon::services::project_namespace::NamespaceWindow {
                name: "main".into(),
                window_id: None,
                agents: vec!["agent1".into()],
            }],
            agent_panes: std::collections::HashMap::new(),
            active_panes: Vec::new(),
            namespace_epoch: 1,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .unwrap();
}

#[test]
fn test_reload_dry_run_no_change() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["plan_class"].as_str(), Some("no_change"));
    assert!(result["operations"].as_array().unwrap().is_empty());
    assert_eq!(result["future_safe_to_apply"].as_bool(), Some(true));
}

#[test]
fn test_reload_dry_run_add_agent() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex; agent2:claude"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["plan_class"].as_str(), Some("add_agent"));
    let ops = result["operations"].as_array().unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["op"].as_str(), Some("add_agent"));
    assert_eq!(ops[0]["agent"].as_str(), Some("agent2"));
    assert_eq!(result["future_safe_to_apply"].as_bool(), Some(true));
}

#[test]
fn test_reload_dry_run_remove_agent() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex; agent2:claude"
"#,
    );
    let mut app = stub_app(&dir);

    write_config(&dir, &base_config());

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["plan_class"].as_str(), Some("remove_agent"));
    let ops = result["operations"].as_array().unwrap();
    let remove_ops: Vec<_> = ops
        .iter()
        .filter(|o| o["op"].as_str() == Some("remove_agent"))
        .collect();
    assert_eq!(remove_ops.len(), 1);
    assert_eq!(remove_ops[0]["agent"].as_str(), Some("agent2"));
    assert_eq!(result["future_safe_to_apply"].as_bool(), Some(true));
}

#[test]
fn test_reload_dry_run_add_window() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex"
secondary = "agent2:claude"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["plan_class"].as_str(), Some("add_window"));
    let ops = result["operations"].as_array().unwrap();
    let add_window_ops: Vec<_> = ops
        .iter()
        .filter(|o| o["op"].as_str() == Some("add_window"))
        .collect();
    assert_eq!(add_window_ops.len(), 1);
    assert_eq!(add_window_ops[0]["window"].as_str(), Some("secondary"));
}

#[test]
fn test_reload_dry_run_replace_agent_is_unsafe() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "claude"
target = "agent1"

[windows]
main = "agent1:claude"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["plan_class"].as_str(), Some("replace_agent"));
    assert_eq!(result["future_safe_to_apply"].as_bool(), Some(false));
    let patch = &result["namespace_patch_plan"];
    assert_eq!(patch["status"].as_str(), Some("blocked"));
    assert!(!patch["blocked_operations"].as_array().unwrap().is_empty());
}

#[test]
fn test_reload_apply_add_agent_updates_registry() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex; agent2:claude"
"#,
    );

    assert!(app.registry.get("agent2").is_none());

    mount_stub_namespace(&mut app);

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("published"));
    assert_eq!(result["plan_class"].as_str(), Some("add_agent"));
    assert_eq!(result["dry_run"].as_bool(), Some(false));
    assert_eq!(result["mutation_enabled"].as_bool(), Some(true));
    assert_eq!(result["safe_to_apply"].as_bool(), Some(true));
    assert!(result["operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|operation| operation["op"] == "add_agent" && operation["agent"] == "agent2"));
    assert_eq!(result["errors"], json!([]));

    let entry = app
        .registry
        .get("agent2")
        .expect("agent2 should be registered");
    assert_eq!(entry.provider, "claude");
    assert_eq!(entry.state, "registered");
    assert!(app.dispatcher.agent_names.contains(&"agent2".to_string()));
}

#[test]
fn test_reload_apply_remove_idle_agent_updates_registry() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex; agent2:claude"
"#,
    );
    let mut app = stub_app(&dir);
    assert!(app.registry.get("agent2").is_some());

    mount_stub_namespace(&mut app);

    write_config(&dir, &base_config());

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("published"));
    assert_eq!(result["plan_class"].as_str(), Some("remove_agent"));
    assert_eq!(result["dry_run"].as_bool(), Some(false));
    assert_eq!(result["mutation_enabled"].as_bool(), Some(true));
    assert_eq!(result["safe_to_apply"].as_bool(), Some(true));
    assert!(result["operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|operation| operation["op"] == "remove_agent" && operation["agent"] == "agent2"));

    assert!(app.registry.get("agent2").is_none());
    assert!(!app.dispatcher.agent_names.contains(&"agent2".to_string()));
}

#[test]
fn test_reload_apply_rejects_unsafe_plan() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "claude"
target = "agent1"

[windows]
main = "agent1:claude"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("blocked"));
    assert_eq!(result["mutation_enabled"].as_bool(), Some(false));
    assert_eq!(result["safe_to_apply"].as_bool(), Some(false));
    let blocker = result["diagnostics"]["reason"].as_str().unwrap();
    assert!(
        blocker == "unsupported_plan_class" || blocker == "plan_not_future_safe",
        "unexpected blocker: {blocker}"
    );
}

#[test]
fn test_reload_apply_add_window_updates_namespace() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    // Seed a namespace so scope verification passes.
    mount_stub_namespace(&mut app);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex"
secondary = "agent2:claude"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("published"));
    assert_eq!(result["plan_class"].as_str(), Some("add_window"));
    assert_eq!(result["mutation_enabled"].as_bool(), Some(true));
    assert!(result["operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|operation| operation["op"] == "add_window" && operation["window"] == "secondary"));

    let ns = app
        .project_namespace
        .load()
        .expect("namespace should exist");
    let window_names: Vec<String> = ns.windows.iter().map(|w| w.name.clone()).collect();
    assert!(window_names.contains(&"secondary".to_string()));
}

#[test]
fn test_reload_apply_remove_busy_agent_is_rejected() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex; agent2:claude"
"#,
    );
    let mut app = stub_app(&dir);

    mount_stub_namespace(&mut app);

    // Simulate a busy agent2; Python blocks removal before namespace mutation.
    app.registry.update_pane_id("agent2", "%42");
    app.registry.set_state("agent2", "busy", "healthy");

    write_config(&dir, &base_config());

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("blocked"));
    assert_eq!(result["plan_class"].as_str(), Some("remove_agent"));
    assert_eq!(result["stage"].as_str(), Some("plan"));
    assert_eq!(result["mutation_enabled"].as_bool(), Some(false));
    assert_eq!(result["diagnostics"]["reason"].as_str(), Some("agent_busy"));
    assert!(app.registry.get("agent2").is_some());
}

#[test]
fn test_reload_invalid_new_config_returns_invalid_plan() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    // Write an invalid config (missing provider).
    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
target = "agent1"

[windows]
main = "agent1"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": true}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("invalid_config"));
    assert!(!result["errors"].as_array().unwrap().is_empty());
    assert_eq!(result["future_safe_to_apply"].as_bool(), Some(false));
}

#[test]
fn test_reload_non_dry_run_invalid_config_uses_python_error_shape() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
target = "agent1"

[windows]
main = "agent1"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("invalid_config"));
    assert_eq!(result["dry_run"].as_bool(), Some(false));
    assert_eq!(result["mutation_enabled"].as_bool(), Some(false));
    assert_eq!(result["safe_to_apply"].as_bool(), Some(false));
    assert_eq!(
        result["diagnostics"]["reason"].as_str(),
        Some("invalid_config")
    );
    assert_eq!(
        result["diagnostics"]["graph_published"].as_bool(),
        Some(false)
    );
}
