use std::collections::HashMap;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use ccb_provider_profiles::{
    codex_api_authority, codex_provider_authority_fingerprint, load_resolved_provider_profile,
    materialize_codex_home_config, materialize_provider_profile,
    materialize_provider_profile_with_source, provider_api_env_keys, provider_api_shortcut_env,
    supported_provider_api_shortcuts, validate_provider_runtime_home_policy,
    validate_provider_runtime_home_uniqueness, ProviderProfileSpec, ResolvedProviderProfile,
};
use ccb_storage::paths::PathLayout;

fn tmp_path(tmp: &tempfile::TempDir, relative: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(tmp.path().join(relative)).unwrap()
}

fn write_codex_plugin_source(home: &Utf8Path, sha: &str) {
    let plugin_root = home.join(".tmp/plugins");
    fs::create_dir_all(plugin_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".agents/skills/plugin-creator")).unwrap();
    fs::create_dir_all(plugin_root.join("plugins/demo-plugin/.codex-plugin")).unwrap();
    fs::create_dir_all(plugin_root.join("plugins/demo-plugin/skills/demo-plugin")).unwrap();
    fs::create_dir_all(home.join(".tmp")).unwrap();
    fs::write(home.join(".tmp/plugins.sha"), format!("{}\n", sha)).unwrap();
    fs::write(
        plugin_root.join(".agents/plugins/marketplace.json"),
        r#"{"name":"openai-curated","plugins":[{"name":"demo-plugin","source":{"source":"local","path":"./plugins/demo-plugin"}}]}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join("plugins/demo-plugin/.codex-plugin/plugin.json"),
        r#"{"name":"demo-plugin"}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join("plugins/demo-plugin/skills/demo-plugin/SKILL.md"),
        "plugin skill\n",
    )
    .unwrap();
}

#[test]
fn test_provider_profile_spec_serde_round_trip() {
    let spec = ProviderProfileSpec {
        mode: "isolated".into(),
        home: Some("/tmp/home".into()),
        env: [("KEY".into(), "value".into())].into(),
        inherit_api: false,
        inherit_auth: true,
        inherit_config: true,
        inherit_skills: true,
        inherit_commands: true,
        inherit_memory: true,
    };
    let record = spec.to_record();
    let json = serde_json::to_string(&record).unwrap();
    let direct: ProviderProfileSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(direct.mode, spec.mode);
    assert_eq!(direct.home, spec.home);
    assert_eq!(direct.inherit_api, spec.inherit_api);
    assert_eq!(direct.env, spec.env);

    let round_record = ProviderProfileSpec::default().to_record();
    assert_eq!(
        round_record.get("mode"),
        Some(&serde_json::Value::String("inherit".into()))
    );
    assert_eq!(
        round_record.get("inherit_api"),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn test_resolved_profile_round_trip() {
    let profile = ResolvedProviderProfile::new("codex", "agent1");
    let record = profile.to_record();
    let obj = record.as_object().unwrap().clone();
    let restored = ResolvedProviderProfile::from_record(&obj).unwrap();
    assert_eq!(profile, restored);
}

#[test]
fn test_supported_provider_api_shortcuts_are_sorted() {
    let supported = supported_provider_api_shortcuts();
    assert_eq!(supported, vec!["claude", "codex", "gemini"]);
}

#[test]
fn test_provider_api_shortcut_env_normalizes_codex_url() {
    let env = provider_api_shortcut_env("codex", Some("my-key"), Some("https://api.example.test/"))
        .unwrap();
    assert_eq!(env.get("OPENAI_API_KEY"), Some(&"my-key".to_string()));
    assert_eq!(
        env.get("OPENAI_BASE_URL"),
        Some(&"https://api.example.test/v1".to_string())
    );
}

#[test]
fn test_provider_api_shortcut_env_rejects_unsupported_provider() {
    assert!(provider_api_shortcut_env("droid", Some("k"), None).is_err());
}

#[test]
fn test_provider_api_env_keys_filters_by_provider() {
    let keys = provider_api_env_keys("gemini");
    assert!(keys.contains("GEMINI_API_KEY"));
    assert!(keys.contains("GOOGLE_GEMINI_BASE_URL"));
    assert!(!keys.contains("OPENAI_API_KEY"));
}

#[test]
fn test_validate_provider_runtime_home_policy() {
    let codex_with_home = ProviderProfileSpec {
        home: Some("/tmp/home".into()),
        ..Default::default()
    };
    assert!(validate_provider_runtime_home_policy("codex", &codex_with_home).is_ok());

    let claude_with_home = ProviderProfileSpec {
        home: Some("/tmp/home".into()),
        ..Default::default()
    };
    assert!(validate_provider_runtime_home_policy("claude", &claude_with_home).is_err());
}

#[test]
fn test_validate_provider_runtime_home_uniqueness_allows_distinct_homes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(Utf8Path::from_path(tmp.path()).unwrap());
    let spec = ProviderProfileSpec::default();
    let specs = vec![("agent1", "codex", &spec), ("agent2", "codex", &spec)];
    assert!(validate_provider_runtime_home_uniqueness(&layout, specs.into_iter()).is_ok());
}

#[test]
fn test_materialize_codex_profile_copies_inherited_assets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let source_home = tmp_path(&tmp, "system-codex-home");
    fs::create_dir_all(source_home.join("skills")).unwrap();
    fs::create_dir_all(source_home.join("commands")).unwrap();
    fs::write(source_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
    fs::write(
        source_home.join("auth.json"),
        r#"{"OPENAI_API_KEY":"system-key"}"#,
    )
    .unwrap();
    fs::write(source_home.join("skills/demo.md"), "demo skill\n").unwrap();
    fs::write(source_home.join("commands/demo.md"), "demo command\n").unwrap();
    write_codex_plugin_source(&source_home, "plugins-sha-v1");

    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile_with_source(
        &layout,
        "agent1",
        "codex",
        &ProviderProfileSpec {
            mode: "isolated".into(),
            ..Default::default()
        },
        &project_root,
        Some(&source_home),
    )
    .unwrap();

    let runtime_home = profile.runtime_home_path().unwrap();
    assert!(runtime_home.exists());
    assert!(runtime_home.join("config.toml").is_file());
    assert!(runtime_home.join("auth.json").is_file());
    assert!(runtime_home.join("skills/demo.md").is_file());
    assert!(runtime_home.join("commands/demo.md").is_file());
    assert!(runtime_home.join("sessions").is_dir());

    let config_text = fs::read_to_string(runtime_home.join("config.toml")).unwrap();
    assert!(config_text.contains("model = \"gpt-5\""));
}

#[test]
fn test_materialize_codex_profile_disables_external_migration() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let source_home = tmp_path(&tmp, "system-codex-home");
    fs::create_dir_all(&source_home).unwrap();
    fs::write(
        source_home.join("config.toml"),
        "model = \"gpt-5.5\"\n\n[features]\nexternal_migration = true\nmemories = true\n",
    )
    .unwrap();

    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile_with_source(
        &layout,
        "agent1",
        "codex",
        &ProviderProfileSpec {
            mode: "isolated".into(),
            ..Default::default()
        },
        &project_root,
        Some(&source_home),
    )
    .unwrap();

    let config_text =
        fs::read_to_string(profile.runtime_home_path().unwrap().join("config.toml")).unwrap();
    assert!(config_text.contains("model = \"gpt-5.5\""));
    assert!(config_text.contains("memories = true"));
    assert!(config_text.contains("external_migration = false"));
    assert!(!config_text.contains("external_migration = true"));
}

#[test]
fn test_materialize_codex_profile_marks_project_trusted() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let workspace_path = tmp_path(&tmp, "repo-worktree");
    let source_home = tmp_path(&tmp, "system-codex-home");
    fs::create_dir_all(&source_home).unwrap();
    fs::create_dir_all(&workspace_path).unwrap();
    fs::write(source_home.join("config.toml"), "model = \"gpt-5.5\"\n").unwrap();

    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile_with_source(
        &layout,
        "agent1",
        "codex",
        &ProviderProfileSpec {
            mode: "isolated".into(),
            ..Default::default()
        },
        &workspace_path,
        Some(&source_home),
    )
    .unwrap();

    let config_text =
        fs::read_to_string(profile.runtime_home_path().unwrap().join("config.toml")).unwrap();
    assert!(config_text.contains("trust_level = \"trusted\""));
}

#[test]
fn test_materialize_codex_profile_routes_plugins_through_shared_bundle() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let source_home = tmp_path(&tmp, "system-codex-home");
    fs::create_dir_all(&source_home).unwrap();
    fs::write(source_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
    write_codex_plugin_source(&source_home, "profile-plugin-sha");

    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile_with_source(
        &layout,
        "agent1",
        "codex",
        &ProviderProfileSpec {
            mode: "isolated".into(),
            ..Default::default()
        },
        &project_root,
        Some(&source_home),
    )
    .unwrap();

    let runtime_home = profile.runtime_home_path().unwrap();
    let bundle = project_root.join(".ccb/shared-cache/codex/plugin-bundles/profile-plugin-sha");
    assert!(bundle.join(".agents/plugins/marketplace.json").is_file());
    assert!(runtime_home.join(".tmp/plugins").is_symlink());
    assert_eq!(
        fs::read_to_string(runtime_home.join(".tmp/plugins.sha")).unwrap(),
        "profile-plugin-sha\n"
    );
}

#[test]
fn test_materialize_codex_profile_preserves_explicit_runtime_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let explicit_home = tmp_path(&tmp, "explicit-codex-home");
    let source_home = tmp_path(&tmp, "system-codex-home");
    fs::create_dir_all(&source_home).unwrap();
    fs::write(source_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile_with_source(
        &layout,
        "agent1",
        "codex",
        &ProviderProfileSpec {
            mode: "isolated".into(),
            home: Some(explicit_home.to_string()),
            ..Default::default()
        },
        &project_root,
        Some(&source_home),
    )
    .unwrap();

    assert_eq!(
        profile.runtime_home_path().unwrap(),
        explicit_home.as_std_path()
    );
    assert_eq!(
        profile.profile_root_path().unwrap(),
        explicit_home.as_std_path()
    );
    assert!(explicit_home.join("config.toml").is_file());
    assert!(explicit_home.join("sessions").is_dir());
}

#[test]
fn test_materialize_claude_profile_filters_env() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile(
        &layout,
        "agent1",
        "claude",
        &ProviderProfileSpec {
            mode: "inherit".into(),
            env: [
                ("ANTHROPIC_API_KEY".into(), "secret".into()),
                ("UNRELATED".into(), "value".into()),
            ]
            .into(),
            ..Default::default()
        },
        &project_root,
    )
    .unwrap();

    assert_eq!(profile.provider, "claude");
    assert!(profile.env.contains_key("ANTHROPIC_API_KEY"));
    assert!(!profile.env.contains_key("UNRELATED"));
    assert!(profile.runtime_home.is_none());
}

#[test]
fn test_load_resolved_provider_profile_round_trip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo");
    let source_home = tmp_path(&tmp, "system-codex-home");
    fs::create_dir_all(&source_home).unwrap();
    fs::write(source_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    let layout = PathLayout::new(&project_root);

    let profile = materialize_provider_profile_with_source(
        &layout,
        "agent1",
        "codex",
        &ProviderProfileSpec::default(),
        &project_root,
        Some(&source_home),
    )
    .unwrap();

    let runtime_dir = layout.agent_provider_runtime_dir("agent1", "codex");
    let loaded = load_resolved_provider_profile(&runtime_dir).unwrap();
    assert_eq!(profile.provider, loaded.provider);
    assert_eq!(profile.agent_name, loaded.agent_name);
    assert_eq!(profile.mode, loaded.mode);
}

#[test]
fn test_codex_api_authority_and_fingerprint() {
    let mut env = HashMap::new();
    env.insert("OPENAI_BASE_URL".into(), "https://api.example.test".into());
    let spec = ProviderProfileSpec {
        inherit_api: false,
        env,
        ..Default::default()
    };

    let authority = codex_api_authority(Some(&spec)).unwrap();
    assert_eq!(authority.provider_id, "custom");
    assert_eq!(authority.base_url, "https://api.example.test");

    let fingerprint = codex_provider_authority_fingerprint(Some(&spec)).unwrap();
    assert_eq!(fingerprint.len(), 16);
}

#[test]
fn test_materialize_codex_home_config_with_explicit_source() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source_home = tmp_path(&tmp, "source");
    let target_home = tmp_path(&tmp, "target");
    fs::create_dir_all(&source_home).unwrap();
    fs::write(source_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    let config_path = materialize_codex_home_config(
        target_home.as_std_path(),
        Some(&ProviderProfileSpec::default()),
        Some(&source_home),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(config_path, target_home.join("config.toml"));
    assert!(target_home.join("sessions").is_dir());
    let text = fs::read_to_string(config_path).unwrap();
    assert!(text.contains("model = \"gpt-5\""));
}
