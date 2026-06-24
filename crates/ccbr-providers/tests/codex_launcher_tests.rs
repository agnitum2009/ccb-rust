use std::path::PathBuf;

use ccbr_agents::models::{
    AgentSpec, PermissionMode, QueuePolicy, RestoreMode, RuntimeMode, WorkspaceMode,
};
use ccbr_providers::codex::{
    build_start_cmd, prepare_codex_home_overrides_for_test, CodexLaunchContext, CodexStartCommand,
};

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        name: name.to_string(),
        provider: "codex".to_string(),
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
        env: std::collections::HashMap::new(),
        api: Default::default(),
        provider_profile: Default::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
}

fn codex_start_command(restore: bool, auto_permission: bool) -> CodexStartCommand {
    CodexStartCommand {
        restore,
        auto_permission,
        provider_command_template: None,
    }
}

fn prepared_state(runtime_dir: &std::path::Path, agent_name: &str) -> CodexLaunchContext {
    let project_root = runtime_project_root(runtime_dir);
    CodexLaunchContext {
        agent_name: agent_name.to_string(),
        project_root: project_root.to_string_lossy().to_string(),
        workspace_path: project_root.to_string_lossy().to_string(),
        agent_events_path: project_root
            .join(".ccbr")
            .join("agents")
            .join(agent_name)
            .join("events.jsonl")
            .to_string_lossy()
            .to_string(),
    }
}

fn runtime_project_root(runtime_dir: &std::path::Path) -> PathBuf {
    let mut current = runtime_dir;
    while let Some(parent) = current.parent() {
        if current.file_name() == Some(std::ffi::OsStr::new(".ccbr")) {
            return parent.to_path_buf();
        }
        current = parent;
    }
    runtime_dir.parent().unwrap_or(runtime_dir).to_path_buf()
}

#[test]
fn test_codex_launcher_build_start_cmd_uses_agent_scoped_session_root_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp
        .path()
        .join("repo")
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let source_home = tmp.path().join("source-home");
    std::fs::create_dir_all(&source_home).unwrap();
    std::fs::write(source_home.join("config.toml"), "[model]\nname=\"gpt-5\"\n").unwrap();
    unsafe { std::env::set_var("CODEX_HOME", &source_home) };

    let s = spec("agent1");
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap(),
        "sess-default",
        Some(&prepared),
    )
    .unwrap();

    let codex_home = runtime_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("provider-state")
        .join("codex")
        .join("home");
    let session_root = codex_home.join("sessions");
    assert!(cmd.contains(&format!(
        "CODEX_HOME={}",
        shlex_quote(&codex_home.to_string_lossy())
    )));
    assert!(cmd.contains(&format!(
        "CODEX_SESSION_ROOT={}",
        shlex_quote(&session_root.to_string_lossy())
    )));
    assert!(session_root.is_dir());
    assert!(codex_home.is_dir());

    unsafe { std::env::remove_var("CODEX_HOME") };
}

#[test]
fn test_codex_launcher_build_start_cmd_includes_agent_model_shortcut() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-model");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let mut s = spec("agent1");
    s.model = Some("gpt-5".to_string());
    s.startup_args = vec!["--search".to_string()];
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-model",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("codex -c disable_paste_burst=true -m gpt-5 --search"));
}

#[test]
fn test_codex_launcher_build_start_cmd_uses_native_auto_permission_flags() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-auto-permission");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let s = spec("agent1");
    let command = codex_start_command(false, true);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-auto-permission",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("--ask-for-approval never"));
    assert!(cmd.contains("--sandbox danger-full-access"));
    assert!(cmd.contains("--dangerously-bypass-hook-trust"));
    assert!(!cmd.contains("trust_level="));
    assert!(!cmd.contains("approval_policy="));
    assert!(!cmd.contains("sandbox_mode="));
}

#[test]
fn test_codex_launcher_build_start_cmd_skips_hook_trust_bypass_in_safe_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-safe-permission");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let s = spec("agent1");
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-safe-permission",
        Some(&prepared),
    )
    .unwrap();

    assert!(!cmd.contains("--ask-for-approval never"));
    assert!(!cmd.contains("--sandbox danger-full-access"));
    assert!(!cmd.contains("--dangerously-bypass-hook-trust"));
}

#[test]
fn test_codex_launcher_build_start_cmd_requires_launch_context() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-no-context");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let s = spec("agent1");
    let command = codex_start_command(false, false);

    let result = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-no-context",
        None,
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("prepare_launch_context"));
}

#[test]
fn test_codex_launcher_build_start_cmd_uses_materialized_profile_home() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let profile_home = tmp.path().join("codex-profile-home");
    let mut env = std::collections::HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "profile-key".to_string());
    let profile = ccbr_provider_profiles::models::ResolvedProviderProfile {
        provider: "codex".to_string(),
        agent_name: "agent1".to_string(),
        mode: "isolated".to_string(),
        profile_root: Some(profile_home.to_string_lossy().to_string()),
        runtime_home: Some(profile_home.to_string_lossy().to_string()),
        env,
        inherit_api: false,
        inherit_auth: true,
        inherit_config: true,
        inherit_skills: true,
        inherit_commands: true,
        inherit_memory: true,
    };
    let record = profile.to_record();
    std::fs::write(
        runtime_dir.join("provider-profile.json"),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let s = spec("agent1");
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-profile",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("unset OPENAI_API_KEY"));
    assert!(cmd.contains(&format!(
        "CODEX_HOME={}",
        shlex_quote(&profile_home.to_string_lossy())
    )));
    assert!(cmd.contains(&format!(
        "CODEX_SESSION_ROOT={}",
        shlex_quote(&profile_home.join("sessions").to_string_lossy())
    )));
    assert!(cmd.contains(&format!("OPENAI_API_KEY={}", shlex_quote("profile-key"))));
    assert!(profile_home.join("sessions").is_dir());
}

#[test]
fn test_codex_launcher_build_start_cmd_respects_agent_restore_fresh() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-fresh");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let mut s = spec("agent1");
    s.restore_default = RestoreMode::Fresh;
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-fresh",
        Some(&prepared),
    )
    .unwrap();

    assert!(!cmd.contains(" resume "));
}

#[test]
fn test_codex_launcher_build_start_cmd_exports_inherited_api_env() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-inherit-api");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };
    unsafe { std::env::set_var("OPENAI_API_KEY", "inherited-key") };

    let s = spec("agent1");
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-inherit-api",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains(&format!("OPENAI_API_KEY={}", shlex_quote("inherited-key"))));

    unsafe { std::env::remove_var("OPENAI_API_KEY") };
}

#[test]
fn test_codex_launcher_build_start_cmd_uses_agent_scoped_resume_session() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo-codex-resume");
    let runtime_dir = project_root
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let ccbr_dir = project_root.join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        project_root.join(".ccbr").join("ccbr_memory.md"),
        "shared memory\n",
    )
    .unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let s = spec("agent1");
    let command = CodexStartCommand {
        restore: true,
        auto_permission: false,
        provider_command_template: None,
    };

    let prepared = prepared_state(&runtime_dir, "agent1");
    let runtime_utf8 = camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap();
    prepare_codex_home_overrides_for_test(
        &runtime_utf8,
        None,
        true,
        Some(&camino::Utf8PathBuf::from_path_buf(project_root.clone()).unwrap()),
        Some("agent1"),
        Some(&camino::Utf8PathBuf::from_path_buf(project_root.clone()).unwrap()),
        Some(
            &camino::Utf8PathBuf::from_path_buf(
                ccbr_dir.join("agents").join("agent1").join("events.jsonl"),
            )
            .unwrap(),
        ),
        Some(&runtime_utf8.join("codex-memory-projection.json")),
    )
    .unwrap();

    let marker_text =
        std::fs::read_to_string(runtime_dir.join("codex-memory-projection.json")).unwrap();
    let marker: serde_json::Value = serde_json::from_str(&marker_text).unwrap();
    let sha256 = marker["sha256"].as_str().unwrap();

    std::fs::write(
        ccbr_dir.join(".codex-agent1-session"),
        serde_json::json!({
            "codex_session_id": "agent1-session-id",
            "codex_memory_projection_sha256": sha256,
            "codex_start_cmd": "codex resume agent1-session-id",
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        ccbr_dir.join(".codex-agent2-session"),
        serde_json::json!({
            "codex_session_id": "agent2-session-id",
            "codex_memory_projection_sha256": sha256,
            "codex_start_cmd": "codex resume agent2-session-id",
        })
        .to_string(),
    )
    .unwrap();

    let cmd =
        build_start_cmd(&command, &s, &runtime_utf8, "sess-restore", Some(&prepared)).unwrap();

    assert!(cmd.ends_with("resume agent1-session-id"));
    assert!(!cmd.contains("agent2-session-id"));
}

#[test]
fn test_codex_launcher_provider_command_template_wraps_original_resume_command() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo-codex-template");
    let runtime_dir = project_root
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let ccbr_dir = project_root.join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        project_root.join(".ccbr").join("ccbr_memory.md"),
        "shared memory\n",
    )
    .unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let mut s = spec("agent1");
    s.provider_command_template = Some("sandbox=1 {command} omx --madmax".to_string());
    let command = CodexStartCommand {
        restore: true,
        auto_permission: false,
        provider_command_template: Some("sandbox=1 {command} omx --madmax".to_string()),
    };

    let prepared = prepared_state(&runtime_dir, "agent1");
    let runtime_utf8 = camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap();
    prepare_codex_home_overrides_for_test(
        &runtime_utf8,
        None,
        true,
        Some(&camino::Utf8PathBuf::from_path_buf(project_root.clone()).unwrap()),
        Some("agent1"),
        Some(&camino::Utf8PathBuf::from_path_buf(project_root.clone()).unwrap()),
        Some(
            &camino::Utf8PathBuf::from_path_buf(
                ccbr_dir.join("agents").join("agent1").join("events.jsonl"),
            )
            .unwrap(),
        ),
        Some(&runtime_utf8.join("codex-memory-projection.json")),
    )
    .unwrap();

    let marker_text =
        std::fs::read_to_string(runtime_dir.join("codex-memory-projection.json")).unwrap();
    let marker: serde_json::Value = serde_json::from_str(&marker_text).unwrap();
    let sha256 = marker["sha256"].as_str().unwrap();

    std::fs::write(
        ccbr_dir.join(".codex-agent1-session"),
        serde_json::json!({
            "codex_session_id": "agent1-session-id",
            "codex_memory_projection_sha256": sha256,
            "codex_start_cmd": "codex resume agent1-session-id",
        })
        .to_string(),
    )
    .unwrap();

    let cmd = build_start_cmd(
        &command,
        &s,
        &runtime_utf8,
        "sess-template",
        Some(&prepared),
    )
    .unwrap();

    assert!(!cmd.contains("{command}"));
    assert!(cmd.starts_with("export "));
    assert!(cmd.contains(
        "; sandbox=1 codex -c disable_paste_burst=true resume agent1-session-id omx --madmax"
    ));
    assert!(!cmd.contains("sandbox=1 export "));
}

#[test]
fn test_codex_launcher_build_start_cmd_skips_resume_when_explicit_api_authority_changed() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo-codex-authority-change");
    let runtime_dir = project_root
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let ccbr_dir = project_root.join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    unsafe { std::env::remove_var("CODEX_HOME") };

    let mut env = std::collections::HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "profile-key".to_string());
    env.insert(
        "OPENAI_BASE_URL".to_string(),
        "https://api.rootflowai.com".to_string(),
    );
    let profile = ccbr_provider_profiles::models::ResolvedProviderProfile {
        provider: "codex".to_string(),
        agent_name: "agent1".to_string(),
        mode: "isolated".to_string(),
        profile_root: Some(
            tmp.path()
                .join("profile-home")
                .to_string_lossy()
                .to_string(),
        ),
        runtime_home: Some(
            tmp.path()
                .join("profile-home")
                .to_string_lossy()
                .to_string(),
        ),
        env,
        inherit_api: false,
        inherit_auth: false,
        inherit_config: false,
        inherit_skills: true,
        inherit_commands: true,
        inherit_memory: true,
    };
    std::fs::write(
        runtime_dir.join("provider-profile.json"),
        serde_json::to_string_pretty(&profile.to_record()).unwrap(),
    )
    .unwrap();

    std::fs::write(
        ccbr_dir.join(".codex-agent1-session"),
        serde_json::json!({"codex_session_id": "legacy-session-id"}).to_string(),
    )
    .unwrap();

    let s = spec("agent1");
    let command = CodexStartCommand {
        restore: true,
        auto_permission: false,
        provider_command_template: None,
    };
    let prepared = prepared_state(&runtime_dir, "agent1");

    let cmd = build_start_cmd(
        &command,
        &s,
        &camino::Utf8PathBuf::from_path_buf(runtime_dir).unwrap(),
        "sess-authority-change",
        Some(&prepared),
    )
    .unwrap();

    assert!(!cmd.contains("resume legacy-session-id"));
}

#[test]
fn test_codex_launcher_build_start_cmd_api_override_clears_global_route_config() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtime-codex-api-override");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let profile_home = tmp.path().join("codex-profile-home");
    let source_home = tmp.path().join("source-home");
    std::fs::create_dir_all(&source_home).unwrap();
    std::fs::write(
        source_home.join("config.toml"),
        r#"model_provider = "stale"
model = "gpt-5.4-openai-compact"
model_reasoning_effort = "xhigh"
disable_response_storage = true

[projects."/tmp/demo-project"]
trust_level = "trusted"

[model_providers.stale]
name = "stale"
base_url = "https://api.ikuncode.cc/v1"
wire_api = "responses"
requires_openai_auth = true
"#,
    )
    .unwrap();
    std::fs::write(
        source_home.join("auth.json"),
        r#"{"OPENAI_API_KEY":"system-key"}"#,
    )
    .unwrap();
    unsafe { std::env::set_var("CODEX_HOME", &source_home) };

    let mut env = std::collections::HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "profile-key".to_string());
    env.insert(
        "OPENAI_BASE_URL".to_string(),
        "https://api.rootflowai.com".to_string(),
    );
    let profile = ccbr_provider_profiles::models::ResolvedProviderProfile {
        provider: "codex".to_string(),
        agent_name: "agent1".to_string(),
        mode: "isolated".to_string(),
        profile_root: Some(profile_home.to_string_lossy().to_string()),
        runtime_home: Some(profile_home.to_string_lossy().to_string()),
        env,
        inherit_api: false,
        inherit_auth: false,
        inherit_config: false,
        inherit_skills: true,
        inherit_commands: true,
        inherit_memory: true,
    };
    std::fs::write(
        runtime_dir.join("provider-profile.json"),
        serde_json::to_string_pretty(&profile.to_record()).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(&profile_home).unwrap();
    std::fs::write(
        profile_home.join("config.toml"),
        "model_provider = \"stale\"\n",
    )
    .unwrap();
    std::fs::write(
        profile_home.join("auth.json"),
        r#"{"OPENAI_API_KEY":"stale-key"}"#,
    )
    .unwrap();

    let s = spec("agent1");
    let command = codex_start_command(false, false);
    let prepared = prepared_state(&runtime_dir, "agent1");
    let runtime_utf8 = camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap();

    prepare_codex_home_overrides_for_test(
        &runtime_utf8,
        Some(&profile),
        true,
        Some(&camino::Utf8PathBuf::from_path_buf(tmp.path().join("project")).unwrap()),
        Some("agent1"),
        Some(&camino::Utf8PathBuf::from_path_buf(tmp.path().join("workspace")).unwrap()),
        Some(&camino::Utf8PathBuf::from_path_buf(tmp.path().join("events.jsonl")).unwrap()),
        Some(&runtime_utf8.join("codex-memory-projection.json")),
    )
    .unwrap();

    let cmd = build_start_cmd(
        &command,
        &s,
        &runtime_utf8,
        "sess-profile-override",
        Some(&prepared),
    )
    .unwrap();

    assert!(cmd.contains("unset OPENAI_API_KEY"));
    assert!(cmd.contains("unset OPENAI_BASE_URL"));
    assert!(cmd.contains(&format!("OPENAI_API_KEY={}", shlex_quote("profile-key"))));
    assert!(!cmd.contains("OPENAI_BASE_URL=https://api.rootflowai.com"));

    let config_text = std::fs::read_to_string(profile_home.join("config.toml")).unwrap();
    assert!(config_text.contains("model_provider = \"custom\""));
    assert!(config_text.contains("model = \"gpt-5.4-openai-compact\""));
    assert!(config_text.contains("model_reasoning_effort = \"xhigh\""));
    assert!(config_text.contains("disable_response_storage = true"));
    assert!(config_text.contains("/tmp/demo-project"));
    assert!(config_text.contains("https://api.rootflowai.com"));
    assert!(config_text.contains("wire_api = \"responses\""));
    assert!(config_text.contains("requires_openai_auth = false"));
    assert!(config_text.contains("external_migration = false"));
    assert!(!config_text.contains("https://api.ikuncode.cc/v1"));

    unsafe { std::env::remove_var("CODEX_HOME") };
}

#[test]
fn test_codex_launcher_resolves_home_from_legacy_start_cmd_env() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let legacy_home = tmp.path().join("legacy-home");
    let session_file = project.join(".ccbr").join(".codex-agent1-session");
    std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    std::fs::write(
        &session_file,
        serde_json::json!({
            "codex_start_cmd": format!(
                "export CODEX_HOME={}; export CODEX_SESSION_ROOT={}/sessions; codex",
                legacy_home.to_string_lossy(),
                legacy_home.to_string_lossy()
            ),
            "codex_session_id": "legacy-sess"
        })
        .to_string(),
    )
    .unwrap();

    let runtime_utf8 = camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap();
    let env = prepare_codex_home_overrides_for_test(
        &runtime_utf8,
        None,
        false,
        Some(&camino::Utf8PathBuf::from_path_buf(project.clone()).unwrap()),
        Some("agent1"),
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(
        env.get("CODEX_HOME").map(std::path::PathBuf::from),
        Some(legacy_home.clone())
    );
    assert_eq!(
        env.get("CODEX_SESSION_ROOT").map(std::path::PathBuf::from),
        Some(legacy_home.join("sessions"))
    );
}

#[test]
fn test_codex_launcher_migrates_legacy_session_path_to_provider_state_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    let runtime_dir = project
        .join(".ccbr")
        .join("agents")
        .join("agent1")
        .join("provider-runtime")
        .join("codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    let legacy_root = tmp.path().join("legacy-codex-home").join("sessions");
    let log_path = legacy_root.join("rollout-legacy-session.jsonl");
    std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    std::fs::write(&log_path, "{}\n").unwrap();

    let session_file = project.join(".ccbr").join(".codex-agent1-session");
    std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    std::fs::write(
        &session_file,
        serde_json::json!({
            "codex_session_path": log_path.to_string_lossy(),
            "codex_session_id": "legacy-sess"
        })
        .to_string(),
    )
    .unwrap();

    let runtime_utf8 = camino::Utf8PathBuf::from_path_buf(runtime_dir.clone()).unwrap();
    let env = prepare_codex_home_overrides_for_test(
        &runtime_utf8,
        None,
        false,
        Some(&camino::Utf8PathBuf::from_path_buf(project.clone()).unwrap()),
        Some("agent1"),
        None,
        None,
        None,
    )
    .unwrap();

    let expected_home = tmp.path().join("legacy-codex-home").join("home");
    let expected_root = expected_home.join("sessions");
    assert_eq!(
        env.get("CODEX_HOME").map(std::path::PathBuf::from),
        Some(expected_home.clone())
    );
    assert_eq!(
        env.get("CODEX_SESSION_ROOT").map(std::path::PathBuf::from),
        Some(expected_root.clone())
    );
    assert!(expected_root.join("rollout-legacy-session.jsonl").is_file());
    assert!(!log_path.exists());
}

fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let safe = s
        .chars()
        .all(|c| c.is_alphanumeric() || "_-.,/:~=@%".contains(c));
    if safe {
        return s.to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
