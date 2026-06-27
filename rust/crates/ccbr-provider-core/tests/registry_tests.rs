use std::collections::HashSet;

use ccbr_provider_core::contracts::LaunchMode;
use ccbr_provider_core::manifest::{CompletionFamily, CompletionSourceKind, SelectorFamily};
use ccbr_provider_core::pathing::session_filename_for_agent;
use ccbr_provider_core::registry::{
    build_default_provider_manifests, build_default_runtime_launcher_map,
    build_default_session_binding_map,
};

const CORE_PROVIDERS: &[&str] = &["codex", "claude", "gemini"];
const ALL_PROVIDERS: &[&str] = &[
    "codex", "claude", "gemini", "opencode", "droid", "agy", "kimi", "deepseek", "zai",
];

fn provider_set(slice: &[&str]) -> HashSet<String> {
    slice.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_default_session_binding_map_uses_backend_owned_entries() {
    let bindings = build_default_session_binding_map(true);

    assert_eq!(
        bindings.keys().cloned().collect::<HashSet<String>>(),
        provider_set(ALL_PROVIDERS)
    );

    assert_eq!(bindings["codex"].session_id_attr, "codex_session_id");
    assert_eq!(bindings["opencode"].session_path_attr, "session_file");
    assert_eq!(bindings["agy"].session_path_attr, "agy_session_path");
    assert_eq!(bindings["kimi"].session_path_attr, "kimi_session_path");
    assert_eq!(
        bindings["deepseek"].session_path_attr,
        "deepseek_session_path"
    );
    assert_eq!(bindings["zai"].session_path_attr, "zai_session_path");

    // All entries should be self-consistent.
    for (provider, binding) in &bindings {
        assert_eq!(&binding.provider, provider);
        assert!(!binding.session_id_attr.is_empty());
        assert!(!binding.session_path_attr.is_empty());
    }
}

#[test]
fn test_default_runtime_launcher_map_uses_backend_owned_entries() {
    let launchers = build_default_runtime_launcher_map(true);

    assert_eq!(
        launchers.keys().cloned().collect::<HashSet<String>>(),
        provider_set(ALL_PROVIDERS)
    );

    assert_eq!(launchers["codex"].launch_mode, LaunchMode::CodexTmux);
    assert_eq!(launchers["gemini"].launch_mode, LaunchMode::SimpleTmux);
    assert_eq!(launchers["agy"].launch_mode, LaunchMode::SimpleTmux);
    assert_eq!(launchers["kimi"].launch_mode, LaunchMode::SimpleTmux);
    assert_eq!(launchers["deepseek"].launch_mode, LaunchMode::SimpleTmux);
    assert_eq!(launchers["zai"].launch_mode, LaunchMode::SimpleTmux);

    for (provider, launcher) in &launchers {
        assert_eq!(&launcher.provider, provider);
    }
}

#[test]
fn test_session_binding_map_core_only() {
    let bindings = build_default_session_binding_map(false);
    assert_eq!(
        bindings.keys().cloned().collect::<HashSet<String>>(),
        provider_set(CORE_PROVIDERS)
    );
}

#[test]
fn test_runtime_launcher_map_core_only() {
    let launchers = build_default_runtime_launcher_map(false);
    assert_eq!(
        launchers.keys().cloned().collect::<HashSet<String>>(),
        provider_set(CORE_PROVIDERS)
    );
}

#[test]
fn test_default_manifests_assign_expected_completion_families() {
    let manifests = build_default_provider_manifests(true, false);
    let by_provider: std::collections::HashMap<
        String,
        &ccbr_provider_core::manifest::ProviderManifest,
    > = manifests.iter().map(|m| (m.provider.clone(), m)).collect();

    let pane_manifest = |provider: &str| {
        by_provider[provider]
            .completion_manifest_for(&ccbr_provider_core::manifest::RuntimeMode::PaneBacked)
            .unwrap()
    };

    assert_eq!(
        pane_manifest("codex").completion_family,
        CompletionFamily::ProtocolTurn
    );
    assert_eq!(
        pane_manifest("claude").completion_family,
        CompletionFamily::ProtocolTurn
    );
    assert_eq!(
        pane_manifest("gemini").completion_family,
        CompletionFamily::AnchoredSessionStability
    );
    assert_eq!(
        pane_manifest("agy").completion_family,
        CompletionFamily::SessionBoundary
    );
    assert_eq!(
        pane_manifest("kimi").completion_family,
        CompletionFamily::SessionBoundary
    );
    assert_eq!(
        pane_manifest("deepseek").completion_family,
        CompletionFamily::SessionBoundary
    );
    assert_eq!(
        pane_manifest("zai").completion_family,
        CompletionFamily::StructuredResult
    );

    assert_eq!(
        pane_manifest("codex").completion_source_kind,
        CompletionSourceKind::ProtocolEventStream
    );
    assert_eq!(
        pane_manifest("agy").completion_source_kind,
        CompletionSourceKind::SessionEventLog
    );
    assert_eq!(
        pane_manifest("deepseek").completion_source_kind,
        CompletionSourceKind::SessionSnapshot
    );
    assert_eq!(
        pane_manifest("zai").completion_source_kind,
        CompletionSourceKind::StructuredResultStream
    );

    assert!(pane_manifest("codex").supports_exact_completion);
    assert!(pane_manifest("codex").supports_anchor_binding);
    assert!(!pane_manifest("codex").supports_observed_completion);
    assert_eq!(
        pane_manifest("codex").selector_family,
        SelectorFamily::FinalMessage
    );
}

#[test]
fn test_session_filename_for_agent_follows_agent_first_naming() {
    assert_eq!(
        session_filename_for_agent("codex", "writer").unwrap(),
        ".codex-writer-session"
    );
    assert_eq!(
        session_filename_for_agent("codex", "codex").unwrap(),
        ".codex-codex-session"
    );
    assert_eq!(
        session_filename_for_agent("agy", "antigravity").unwrap(),
        ".agy-antigravity-session"
    );
    assert_eq!(
        session_filename_for_agent("kimi", "moon").unwrap(),
        ".kimi-moon-session"
    );
    assert_eq!(
        session_filename_for_agent("deepseek", "coder").unwrap(),
        ".deepseek-coder-session"
    );
    assert_eq!(
        session_filename_for_agent("zai", "agent").unwrap(),
        ".zai-agent-session"
    );
}
