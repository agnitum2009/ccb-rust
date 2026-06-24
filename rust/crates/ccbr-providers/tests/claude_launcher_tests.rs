use std::collections::HashMap;

use camino::Utf8Path;
use ccbr_agents::models::{
    AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccbr_providers::claude::launcher_runtime::history::project_key;
use ccbr_providers::claude::launcher_runtime::resolve_claude_restore_target;
use ccbr_providers::claude::{build_claude_start_cmd, ClaudeStartCommand};

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
        .join(".ccbr")
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
        .join(".ccbr")
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
        .join(".ccbr")
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

fn managed_claude_home(runtime_dir: &std::path::Path) -> std::path::PathBuf {
    // Mirror provider-runtime -> provider-state mapping in resolve_claude_home_layout.
    runtime_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("provider-state")
        .join("claude")
        .join("home")
}

fn seed_claude_history(
    managed_home: &std::path::Path,
    project: &std::path::Path,
    session_id: Option<&str>,
) {
    let projects_root = managed_home.join(".claude").join("projects");
    let project_dir = projects_root.join(project_key(Utf8Path::from_path(project).unwrap()));
    let session_env_root = managed_home.join(".claude").join("session-env");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::create_dir_all(&session_env_root).unwrap();

    let name = session_id.unwrap_or("plain-history");
    std::fs::write(project_dir.join(format!("{name}.jsonl")), "{}\n").unwrap();
    if let Some(uuid) = session_id {
        std::fs::create_dir_all(session_env_root.join(uuid)).unwrap();
    }
}

#[test]
fn test_claude_launcher_build_start_cmd_adds_continue_when_history_found() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    seed_claude_history(
        &managed_claude_home(&runtime_dir),
        &project,
        Some(&session_id),
    );

    let s = spec("agent1");
    let command = ClaudeStartCommand {
        restore: true,
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

    assert!(cmd.contains("--continue"), "cmd: {}", cmd);
}

#[test]
fn test_claude_launcher_build_start_cmd_omits_continue_when_fresh() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    seed_claude_history(
        &managed_claude_home(&runtime_dir),
        &project,
        Some(&session_id),
    );

    let mut s = spec("agent1");
    s.restore_default = RestoreMode::Fresh;
    let command = ClaudeStartCommand {
        restore: true,
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

    assert!(!cmd.contains("--continue"), "cmd: {}", cmd);
}

#[test]
fn test_claude_history_locator_finds_uuid_session() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    let managed_home = managed_claude_home(&runtime_dir);
    seed_claude_history(&managed_home, &project, Some(&session_id));

    let env = HashMap::new();
    let locator = ccbr_providers::claude::launcher_runtime::history::ClaudeHistoryLocator::new(
        Utf8Path::from_path(&project).unwrap(),
        Utf8Path::from_path(&project).unwrap(),
        &env,
        Utf8Path::from_path(&managed_home).unwrap(),
    );
    let (found_id, has_history, best_cwd) = locator.latest_session_id();
    assert_eq!(found_id, Some(session_id));
    assert!(has_history);
    assert!(best_cwd.is_some());
}

#[test]
fn test_claude_history_locator_falls_back_to_any_history() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let managed_home = managed_claude_home(&runtime_dir);
    seed_claude_history(&managed_home, &project, None);

    let env = HashMap::new();
    let locator = ccbr_providers::claude::launcher_runtime::history::ClaudeHistoryLocator::new(
        Utf8Path::from_path(&project).unwrap(),
        Utf8Path::from_path(&project).unwrap(),
        &env,
        Utf8Path::from_path(&managed_home).unwrap(),
    );
    let (found_id, has_history, best_cwd) = locator.latest_session_id();
    assert_eq!(found_id, None);
    assert!(has_history);
    assert!(best_cwd.is_some());
}

#[test]
fn test_resolve_claude_restore_target_returns_history_and_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    let managed_home = managed_claude_home(&runtime_dir);
    seed_claude_history(&managed_home, &project, Some(&session_id));

    let s = spec("agent1");
    let target = resolve_claude_restore_target(
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        true,
        Some(Utf8Path::from_path(&project).unwrap()),
    );
    assert!(target.has_history);
    assert_eq!(target.run_cwd, Utf8Path::from_path(&project).unwrap());
}

#[test]
fn test_resolve_claude_restore_target_fresh_ignores_history() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    let managed_home = managed_claude_home(&runtime_dir);
    seed_claude_history(&managed_home, &project, Some(&session_id));

    let mut s = spec("agent1");
    s.restore_default = RestoreMode::Fresh;
    let target = resolve_claude_restore_target(
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        true,
        Some(Utf8Path::from_path(&project).unwrap()),
    );
    assert!(!target.has_history);
}
