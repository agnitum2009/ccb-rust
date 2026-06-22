use std::collections::HashMap;

use ccb_agents::models::{
    AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccb_providers::claude::{build_claude_start_cmd, ClaudeStartCommand};

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        name: name.to_string(),
        provider: "claude".to_string(),
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
        "agent_events_path".to_string(),
        project_root
            .join("events.jsonl")
            .to_string_lossy()
            .to_string(),
    );
    state.insert(
        "workspace_path".to_string(),
        project_root.to_string_lossy().to_string(),
    );
    state
}

#[test]
fn test_claude_launcher_build_start_cmd_requires_project_root() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("rt");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let s = spec("agent1");
    let command = ClaudeStartCommand::default();
    let err = build_claude_start_cmd(
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
fn test_claude_launcher_build_start_cmd_includes_home_overrides_and_env_prefix() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = ClaudeStartCommand::default();
    let prepared = prepared_state(&project);
    let cmd = build_claude_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap(),
        "sess-1",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("HOME="), "cmd: {}", cmd);
    assert!(cmd.contains("CLAUDE_PROJECTS_ROOT="), "cmd: {}", cmd);
    assert!(cmd.contains("CLAUDE_PROJECT_ROOT="), "cmd: {}", cmd);
    assert!(
        cmd.contains("--setting-sources user,project,local"),
        "cmd: {}",
        cmd
    );
    assert!(cmd.contains("CCB_CALLER_ACTOR=agent1"), "cmd: {}", cmd);
    assert!(cmd.contains(" claude "), "cmd: {}", cmd);
}

#[test]
fn test_claude_launcher_build_start_cmd_auto_permission_adds_bypass_and_skip_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let s = spec("agent1");
    let command = ClaudeStartCommand {
        auto_permission: true,
        ..Default::default()
    };
    let prepared = prepared_state(&project);
    let cmd = build_claude_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap(),
        "sess-1",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("--permission-mode bypassPermissions"));
    let settings_path = runtime_dir.join("claude-settings.json");
    assert!(settings_path.is_file());
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(
        payload.get("skipDangerousModePermissionPrompt"),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn test_claude_launcher_build_start_cmd_applies_provider_command_template() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let mut s = spec("agent1");
    s.startup_args = vec!["--verbose".to_string()];
    let command = ClaudeStartCommand {
        provider_command_template: Some(
            "tmux-sideloader --agent agent1 -- '{command}'".to_string(),
        ),
        ..Default::default()
    };
    let prepared = prepared_state(&project);
    let cmd = build_claude_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-1",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("tmux-sideloader --agent agent1 -- '"));
    assert!(cmd.contains(" --verbose"));
}
