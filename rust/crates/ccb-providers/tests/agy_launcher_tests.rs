use std::collections::HashMap;

use ccb_agents::models::{
    AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccb_providers::providers::agy::{build_agy_start_cmd, AgyStartCommand};

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        name: name.to_string(),
        provider: "agy".to_string(),
        target: ".".to_string(),
        workspace_mode: WorkspaceMode::GitWorktree,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
        restore_default: RestoreMode::Auto,
        permission_default: PermissionMode::Manual,
        queue_policy: QueuePolicy::SerialPerAgent,
        workspace_path: None,
        workspace_group: None,
        provider_command_template: None,
        model: None,
        startup_args: Vec::new(),
        env: HashMap::new(),
        api: Default::default(),
        provider_profile: Default::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
}

#[test]
fn test_agy_launcher_build_start_cmd_includes_home_override_and_context() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp
        .path()
        .join("project")
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("agy");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = AgyStartCommand::default();
    let cmd = build_agy_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap(),
        "sess-1",
        None,
    )
    .unwrap();

    assert!(cmd.contains("HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("USERPROFILE="), "cmd: {}", cmd);
    assert!(cmd.contains("CCB_CALLER_ACTOR=agent1"), "cmd: {}", cmd);
    assert!(cmd.contains(" agy"), "cmd: {}", cmd);
}

#[test]
fn test_agy_launcher_build_start_cmd_auto_permission_and_restore() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = AgyStartCommand {
        auto_permission: true,
        restore: true,
        ..Default::default()
    };
    let cmd = build_agy_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        None,
    )
    .unwrap();

    assert!(
        cmd.contains("--dangerously-skip-permissions"),
        "cmd: {}",
        cmd
    );
    assert!(cmd.contains("--continue"), "cmd: {}", cmd);
}

#[test]
fn test_agy_launcher_build_start_cmd_applies_provider_command_template() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let mut s = spec("agent1");
    s.startup_args = vec!["--verbose".to_string()];
    let command = AgyStartCommand {
        provider_command_template: Some("wrapper '{command}'".to_string()),
        ..Default::default()
    };
    let cmd = build_agy_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        None,
    )
    .unwrap();

    assert!(cmd.contains("wrapper '"), "cmd: {}", cmd);
    assert!(cmd.contains("agy --verbose"), "cmd: {}", cmd);
}
