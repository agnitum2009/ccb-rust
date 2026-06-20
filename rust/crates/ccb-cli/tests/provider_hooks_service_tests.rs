//! Mirrors Python `test/test_provider_hook_settings.py` for the orchestrator
//! `prepare_workspace_provider_hooks`.

use std::io::Write;

use camino::Utf8Path;
use ccb_agents::models::{
    AgentSpec, PermissionMode, ProviderProfileSpec, QueuePolicy, RestoreMode, RuntimeMode,
    WorkspaceMode,
};

use ccb_cli::provider_hooks::{
    prepare_provider_workspace, prepare_workspace_provider_hooks, provider_hook_home_root,
    resolve_gemini_home_root,
};

fn tmp_dir() -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let utf8 = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    (dir, utf8)
}

fn make_hook_bin(dir: &Utf8Path, name: &str) -> camino::Utf8PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"#!/bin/sh\necho hook\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
    }
    path
}

fn read_settings(path: &Utf8Path) -> serde_json::Map<String, serde_json::Value> {
    let text = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn test_prepare_hooks_unsupported_provider_returns_none() {
    let (_tmp, workspace) = tmp_dir();
    let result = prepare_workspace_provider_hooks(
        "droid",
        &workspace,
        &workspace,
        "agent1",
        Some(&workspace),
        None,
        None,
        None,
    );
    assert!(result.is_none());
}

#[test]
fn test_prepare_hooks_claude_installs_completion_hooks() {
    let (_tmp, root) = tmp_dir();
    let workspace = root.join("workspace");
    let completion_dir = root.join("completion");
    let home_root = root.join("home");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&completion_dir).unwrap();
    std::fs::create_dir_all(&home_root).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();
    make_hook_bin(&bin_dir, "ccb-provider-finish-hook");

    std::env::set_var("CCB_HOOK_BIN_DIR", bin_dir.as_str());
    let result = prepare_workspace_provider_hooks(
        "claude",
        &workspace,
        &completion_dir,
        "agent1",
        Some(&home_root),
        None,
        None,
        None,
    );

    let settings_path = home_root.join(".claude").join("settings.json");
    assert_eq!(result, Some(settings_path.clone()));
    let settings = read_settings(&settings_path);
    let stop = settings
        .get("hooks")
        .unwrap()
        .get("Stop")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(stop.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .unwrap_or(&Vec::new())
            .iter()
            .any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .contains("ccb-provider-finish-hook")
            })
    }));
}

#[test]
fn test_prepare_hooks_claude_installs_activity_hooks_with_project_and_runtime() {
    let (_tmp, root) = tmp_dir();
    let workspace = root.join("workspace");
    let completion_dir = root.join("completion");
    let home_root = root.join("home");
    let runtime_dir = root.join("runtime");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&completion_dir).unwrap();
    std::fs::create_dir_all(&home_root).unwrap();
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();
    make_hook_bin(&bin_dir, "ccb-provider-finish-hook");
    make_hook_bin(&bin_dir, "ccb-provider-activity-hook");

    std::env::set_var("CCB_HOOK_BIN_DIR", bin_dir.as_str());
    let result = prepare_workspace_provider_hooks(
        "claude",
        &workspace,
        &completion_dir,
        "agent1",
        Some(&home_root),
        Some("proj1"),
        Some(&runtime_dir),
        None,
    );

    let settings_path = home_root.join(".claude").join("settings.json");
    assert_eq!(result, Some(settings_path.clone()));
    let settings = read_settings(&settings_path);
    let hooks = settings.get("hooks").unwrap();

    fn has_command(arr: &[serde_json::Value], needle: &str) -> bool {
        arr.iter().any(|entry| {
            entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .unwrap_or(&Vec::new())
                .iter()
                .any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .contains(needle)
                })
        })
    }

    let events = ["SessionStart", "UserPromptSubmit", "Stop"];
    for event in events {
        let arr = hooks.get(event).unwrap().as_array().unwrap();
        assert!(
            has_command(arr, "ccb-provider-activity-hook"),
            "event {event} missing activity hook"
        );
    }

    // Stop should still contain the finish hook too.
    let stop = hooks.get("Stop").unwrap().as_array().unwrap();
    assert!(has_command(stop, "ccb-provider-finish-hook"));
}

#[test]
fn test_prepare_hooks_gemini_installs_completion_hooks() {
    let (_tmp, root) = tmp_dir();
    let workspace = root.join("workspace");
    let completion_dir = root.join("completion");
    let home_root = root.join("home");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&completion_dir).unwrap();
    std::fs::create_dir_all(&home_root).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();
    make_hook_bin(&bin_dir, "ccb-provider-finish-hook");

    std::env::set_var("CCB_HOOK_BIN_DIR", bin_dir.as_str());
    let result = prepare_workspace_provider_hooks(
        "gemini",
        &workspace,
        &completion_dir,
        "agent1",
        Some(&home_root),
        Some("proj1"),
        Some(&workspace),
        None,
    );

    let settings_path = home_root.join(".gemini").join("settings.json");
    assert_eq!(result, Some(settings_path.clone()));
    let settings = read_settings(&settings_path);
    let hooks = settings.get("hooks").unwrap();
    let after_agent = hooks.get("AfterAgent").unwrap().as_array().unwrap();
    assert!(after_agent.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .unwrap_or(&Vec::new())
            .iter()
            .any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .contains("ccb-provider-finish-hook")
            })
    }));

    // Gemini should not get activity hooks even with project_id + runtime_dir.
    assert!(!hooks
        .as_object()
        .unwrap()
        .values()
        .any(|v| v.to_string().contains("ccb-provider-activity-hook")));
}

#[test]
fn test_resolve_gemini_home_root() {
    let (_tmp, root) = tmp_dir();
    let layout = ccb_storage::paths::PathLayout::new(root);
    let expected = layout
        .agent_provider_state_dir("agent1", "gemini")
        .join("home");
    assert_eq!(resolve_gemini_home_root(&layout, "agent1"), expected);
}

#[test]
fn test_provider_hook_home_root_claude_uses_runtime_home() {
    let (_tmp, root) = tmp_dir();
    let runtime_dir = root.join("runtime");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let layout = ccb_storage::paths::PathLayout::new(&root);

    let home_root = provider_hook_home_root(&layout, "claude", "agent1", &runtime_dir, None);

    assert_eq!(home_root, Some(runtime_dir.join("home")));
}

#[test]
fn test_provider_hook_home_root_gemini() {
    let (_tmp, root) = tmp_dir();
    let runtime_dir = root.join("runtime");
    let layout = ccb_storage::paths::PathLayout::new(&root);

    let home_root = provider_hook_home_root(&layout, "gemini", "agent1", &runtime_dir, None);

    assert_eq!(
        home_root,
        Some(
            layout
                .agent_provider_state_dir("agent1", "gemini")
                .join("home")
        )
    );
}

fn base_spec(name: &str, provider: &str) -> AgentSpec {
    let mut spec = claude_spec(name);
    spec.provider = provider.into();
    if provider == "codex" {
        spec.provider_profile.mode = "isolated".into();
    }
    spec
}

fn claude_spec(name: &str) -> AgentSpec {
    AgentSpec {
        name: name.into(),
        provider: "claude".into(),
        target: ".".into(),
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
        env: Default::default(),
        api: Default::default(),
        provider_profile: ProviderProfileSpec::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
}

#[test]
fn test_prepare_provider_workspace_materializes_claude_settings_before_hooks() {
    let (_tmp, root) = tmp_dir();
    let project_root = root.join("repo");
    let workspace = project_root.join("workspace");
    let system_home = root.join("system-home");
    let system_settings = system_home.join(".claude").join("settings.json");
    std::fs::create_dir_all(system_settings.parent().unwrap()).unwrap();
    std::fs::write(
        &system_settings,
        serde_json::json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "system-token",
                "ANTHROPIC_BASE_URL": "https://claude.example.test",
            },
            "theme": "light",
        })
        .to_string(),
    )
    .unwrap();

    // Point provider source home at the synthetic system home.
    std::env::set_var("HOME", system_home.as_str());
    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = ccb_storage::paths::PathLayout::new(&project_root);
    let completion_dir = layout
        .agent_provider_runtime_dir("agent1", "claude")
        .join("completion");

    let profile = prepare_provider_workspace(
        &layout,
        &claude_spec("agent1"),
        &workspace,
        &completion_dir,
        "agent1",
        true,
        false,
    )
    .unwrap();

    assert_eq!(profile.provider, "claude");
    assert_eq!(profile.agent_name, "agent1");

    let settings_path = layout
        .agent_provider_state_dir("agent1", "claude")
        .join("home")
        .join(".claude")
        .join("settings.json");
    assert!(
        settings_path.exists(),
        "settings missing at {settings_path}"
    );
    assert!(settings_path.exists());
    let settings = read_settings(&settings_path);
    assert_eq!(
        settings
            .get("env")
            .unwrap()
            .get("ANTHROPIC_AUTH_TOKEN")
            .unwrap()
            .as_str()
            .unwrap(),
        "system-token"
    );
}

#[test]
fn test_prepare_provider_workspace_materializes_codex_home() {
    let (_tmp, root) = tmp_dir();
    let project_root = root.join("repo");
    let workspace = project_root.join("workspace");
    let system_home = root.join("system-codex-home");
    std::fs::create_dir_all(&system_home).unwrap();
    std::fs::write(system_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    std::env::set_var("HOME", system_home.as_str());
    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = ccb_storage::paths::PathLayout::new(&project_root);
    let completion_dir = layout
        .agent_provider_runtime_dir("agent1", "codex")
        .join("completion");

    let mut spec = claude_spec("agent1");
    spec.provider = "codex".into();
    spec.provider_profile.mode = "isolated".into();

    let profile = prepare_provider_workspace(
        &layout,
        &spec,
        &workspace,
        &completion_dir,
        "agent1",
        true,
        false,
    )
    .unwrap();

    assert_eq!(profile.provider, "codex");

    let config_path = layout
        .agent_provider_state_dir("agent1", "codex")
        .join("home")
        .join("config.toml");
    assert!(config_path.exists());
    let text = std::fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("model = \"gpt-5\""));
}

#[test]
fn test_prepare_provider_workspace_materializes_gemini_home() {
    let (_tmp, root) = tmp_dir();
    let project_root = root.join("repo");
    let workspace = project_root.join("workspace");
    let system_home = root.join("system-home");
    std::fs::create_dir_all(&system_home).unwrap();

    std::env::set_var("HOME", system_home.as_str());
    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = ccb_storage::paths::PathLayout::new(&project_root);
    let completion_dir = layout
        .agent_provider_runtime_dir("agent1", "gemini")
        .join("completion");

    let spec = base_spec("agent1", "gemini");

    let profile = prepare_provider_workspace(
        &layout,
        &spec,
        &workspace,
        &completion_dir,
        "agent1",
        true,
        false,
    )
    .unwrap();

    assert_eq!(profile.provider, "gemini");

    let gemini_dir = layout
        .agent_provider_state_dir("agent1", "gemini")
        .join("home")
        .join(".gemini");
    assert!(gemini_dir.is_dir());
    assert!(gemini_dir.join("trustedFolders.json").exists());
}

#[test]
fn test_prepare_provider_workspace_materializes_opencode_config() {
    let (_tmp, root) = tmp_dir();
    let project_root = root.join("repo");
    let workspace = project_root.join("workspace");

    std::env::remove_var("CCB_SOURCE_HOME");

    let layout = ccb_storage::paths::PathLayout::new(&project_root);
    let completion_dir = layout
        .agent_provider_runtime_dir("agent1", "opencode")
        .join("completion");

    let spec = base_spec("agent1", "opencode");

    let profile = prepare_provider_workspace(
        &layout,
        &spec,
        &workspace,
        &completion_dir,
        "agent1",
        true,
        false,
    )
    .unwrap();

    assert_eq!(profile.provider, "opencode");

    let config_path = layout
        .agent_provider_state_dir("agent1", "opencode")
        .join("opencode.json");
    assert!(config_path.exists());
}

#[test]
fn test_provider_hook_home_root_unsupported_returns_none() {
    let (_tmp, root) = tmp_dir();
    let runtime_dir = root.join("runtime");
    let layout = ccb_storage::paths::PathLayout::new(&root);

    let home_root = provider_hook_home_root(&layout, "droid", "agent1", &runtime_dir, None);

    assert!(home_root.is_none());
}
