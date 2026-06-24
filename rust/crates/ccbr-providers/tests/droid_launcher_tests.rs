use std::collections::HashMap;

use ccbr_agents::models::{
    AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccbr_providers::droid::launcher::{build_start_cmd, DroidStartCommand};

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        name: name.to_string(),
        provider: "droid".to_string(),
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
fn test_droid_launcher_build_start_cmd_includes_home_env_and_caller_context() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp
        .path()
        .join("project")
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("droid");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = DroidStartCommand::default();
    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap(),
        "sess-1",
        None,
    )
    .unwrap();

    assert!(cmd.contains("FACTORY_HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("FACTORY_SESSIONS_ROOT="), "cmd: {}", cmd);
    assert!(cmd.contains("DROID_SESSIONS_ROOT="), "cmd: {}", cmd);
    assert!(cmd.contains("CCB_CALLER_ACTOR=agent1"), "cmd: {}", cmd);
    assert!(cmd.contains(" droid"), "cmd: {}", cmd);
}

#[test]
fn test_droid_launcher_build_start_cmd_restore_adds_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = DroidStartCommand {
        restore: true,
        ..Default::default()
    };
    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        None,
    )
    .unwrap();

    assert!(cmd.contains(" droid -r"), "cmd: {}", cmd);
}

#[test]
fn test_droid_launcher_build_start_cmd_uses_prepared_home_and_sessions_root() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let mut prepared = HashMap::new();
    prepared.insert(
        "droid_home".to_string(),
        tmp.path()
            .join("custom-droid-home")
            .to_string_lossy()
            .to_string(),
    );
    prepared.insert(
        "droid_sessions_root".to_string(),
        tmp.path()
            .join("custom-sessions")
            .to_string_lossy()
            .to_string(),
    );

    let s = spec("agent1");
    let command = DroidStartCommand::default();
    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("FACTORY_HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("custom-droid-home"), "cmd: {}", cmd);
    assert!(cmd.contains("FACTORY_SESSIONS_ROOT="), "cmd: {}", cmd);
    assert!(cmd.contains("DROID_SESSIONS_ROOT="), "cmd: {}", cmd);
    assert!(cmd.contains("custom-sessions"), "cmd: {}", cmd);
}

#[test]
fn test_droid_launcher_build_start_cmd_applies_provider_command_template() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let mut s = spec("agent1");
    s.startup_args = vec!["--verbose".to_string()];
    let command = DroidStartCommand {
        provider_command_template: Some("wrapper '{command}'".to_string()),
        ..Default::default()
    };
    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        None,
    )
    .unwrap();

    assert!(cmd.contains("wrapper '"), "cmd: {}", cmd);
    assert!(cmd.contains("droid --verbose"), "cmd: {}", cmd);
}
