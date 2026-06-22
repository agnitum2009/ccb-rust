use std::collections::HashMap;

use ccb_agents::models::{
    AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccb_providers::providers::gemini::{build_gemini_start_cmd, GeminiStartCommand};

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        name: name.to_string(),
        provider: "gemini".to_string(),
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

fn prepared_state(project_root: &std::path::Path) -> HashMap<String, String> {
    let mut state = HashMap::new();
    state.insert(
        "project_root".to_string(),
        project_root.to_string_lossy().to_string(),
    );
    state.insert(
        "workspace_path".to_string(),
        project_root.to_string_lossy().to_string(),
    );
    state
}

#[test]
fn test_gemini_launcher_build_start_cmd_requires_project_root() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let s = spec("agent1");
    let command = GeminiStartCommand::default();
    let err = build_gemini_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("prepare_launch_context"));
}

#[test]
fn test_gemini_launcher_build_start_cmd_includes_home_env_and_context() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("gemini");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = GeminiStartCommand::default();
    let cmd = build_gemini_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        Some(&prepared_state(&project)),
    )
    .unwrap();

    assert!(cmd.contains("HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("GEMINI_CLI_HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("GEMINI_ROOT="), "cmd: {}", cmd);
    assert!(cmd.contains("NPM_CONFIG_CACHE="), "cmd: {}", cmd);
    assert!(cmd.contains("XDG_CACHE_HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("CCB_CALLER_ACTOR=agent1"), "cmd: {}", cmd);
    assert!(cmd.contains(" gemini"), "cmd: {}", cmd);
}

#[test]
fn test_gemini_launcher_build_start_cmd_auto_permission_and_restore() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project.join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = GeminiStartCommand {
        auto_permission: true,
        restore: true,
        ..Default::default()
    };
    let cmd = build_gemini_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        Some(&prepared_state(&project)),
    )
    .unwrap();

    assert!(cmd.contains("--yolo"), "cmd: {}", cmd);
    assert!(cmd.contains("--resume latest"), "cmd: {}", cmd);
}

#[test]
fn test_gemini_launcher_build_start_cmd_applies_provider_command_template() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project.join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let mut s = spec("agent1");
    s.startup_args = vec!["--verbose".to_string()];
    let command = GeminiStartCommand {
        provider_command_template: Some("wrapper '{command}'".to_string()),
        ..Default::default()
    };
    let cmd = build_gemini_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        Some(&prepared_state(&project)),
    )
    .unwrap();

    assert!(cmd.contains("wrapper '"), "cmd: {}", cmd);
    assert!(cmd.contains("gemini --verbose"), "cmd: {}", cmd);
}
