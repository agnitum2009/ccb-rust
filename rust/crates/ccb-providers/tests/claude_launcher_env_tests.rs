use std::collections::HashMap;

use ccb_provider_profiles::models::ResolvedProviderProfile;
use ccb_providers::claude::launcher_runtime::env::{
    build_claude_env_prefix, claude_user_base_url, write_claude_settings_overlay,
};

#[test]
fn test_build_claude_env_prefix_unsets_dead_local_base_url_from_env() {
    let mut env = HashMap::new();
    env.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        "http://127.0.0.1:12345".to_string(),
    );
    let result = build_claude_env_prefix(None, None, Some(&env), |_| true, String::new);
    assert_eq!(result, "unset ANTHROPIC_BASE_URL");
}

#[test]
fn test_build_claude_env_prefix_uses_settings_base_url_when_inheritable() {
    let result = build_claude_env_prefix(
        None,
        None,
        None,
        |_| false,
        || "https://api.example.test".to_string(),
    );
    assert_eq!(result, "export ANTHROPIC_BASE_URL=https://api.example.test");
}

#[test]
fn test_build_claude_env_prefix_prefers_settings_base_url_over_ambient_env() {
    let mut env = HashMap::new();
    env.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        "https://old-shell.example.test".to_string(),
    );
    let result = build_claude_env_prefix(
        None,
        None,
        Some(&env),
        |_| false,
        || "https://ccswitch.example.test".to_string(),
    );
    assert_eq!(
        result,
        "export ANTHROPIC_BASE_URL=https://ccswitch.example.test"
    );
}

#[test]
fn test_write_claude_settings_overlay_returns_none_without_agent_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    assert!(write_claude_settings_overlay(&runtime_dir, None).is_none());
}

#[test]
fn test_write_claude_settings_overlay_strips_env_section_from_agent_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let profile_root = tmp.path().join("profile");
    let settings_path = profile_root.join("settings.json");
    std::fs::create_dir_all(&profile_root).unwrap();
    std::fs::write(
        &settings_path,
        serde_json::json!({
            "env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:12345"},
            "theme": "light"
        })
        .to_string(),
    )
    .unwrap();

    let profile = ResolvedProviderProfile {
        provider: "claude".to_string(),
        agent_name: "agent1".to_string(),
        mode: "inherit".to_string(),
        profile_root: Some(profile_root.to_string_lossy().to_string()),
        runtime_home: None,
        env: HashMap::new(),
        inherit_api: true,
        inherit_auth: true,
        inherit_config: true,
        inherit_skills: true,
        inherit_commands: true,
        inherit_memory: true,
    };

    let runtime_dir = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let overlay = write_claude_settings_overlay(&runtime_dir, Some(&profile)).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&overlay).unwrap()).unwrap();
    assert_eq!(payload, serde_json::json!({"theme": "light"}));

    let user_settings = camino::Utf8PathBuf::from_path_buf(settings_path).unwrap();
    assert_eq!(
        claude_user_base_url(&user_settings),
        "http://127.0.0.1:12345"
    );
}
