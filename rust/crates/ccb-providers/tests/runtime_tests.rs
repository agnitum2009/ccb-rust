use std::collections::HashMap;

use ccb_providers::runtime::{
    build_runtime_helper_manifest, cleanup_stale_runtime_helper, load_helper_manifest,
    save_helper_manifest, ProgressState, ProviderCompletionState, ProviderHealthSnapshot,
    ProviderHealthSnapshotStore, ProviderHelperManifest, RuntimeInfo,
};
use ccb_storage::paths::PathLayout;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn test_provider_health_snapshot_creation() {
    let snapshot = ProviderHealthSnapshot::new("j1", "claude", "agent1", "2025-01-01T00:00:00Z")
        .with_runtime_alive(true)
        .with_session_reachable(Some(true))
        .with_progress_state(ProgressState::ActivelyRunning)
        .with_completion_state(ProviderCompletionState::NotComplete)
        .with_last_progress_at("2025-01-01T00:00:00Z")
        .with_degraded_reason("slow")
        .with_diagnostics(HashMap::from([(
            "foo".to_string(),
            serde_json::json!("bar"),
        )]));
    assert!(snapshot.runtime_alive);
    assert_eq!(snapshot.progress_state, ProgressState::ActivelyRunning);
    assert_eq!(
        snapshot.completion_state,
        ProviderCompletionState::NotComplete
    );
}

#[test]
#[should_panic(expected = "job_id cannot be empty")]
fn test_provider_health_snapshot_empty_job_id_panics() {
    ProviderHealthSnapshot::new("", "claude", "agent1", "2025-01-01T00:00:00Z");
}

#[test]
fn test_helper_manifest_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("helper.json");
    let utf8_path = camino::Utf8Path::from_path(&path).unwrap();
    let manifest = ProviderHelperManifest::new("agent1", 3, "codex_bridge", 42)
        .with_started_at("2025-01-01T00:00:00Z")
        .with_owner_daemon_generation(Some(7));
    save_helper_manifest(utf8_path, &manifest).unwrap();
    let loaded = load_helper_manifest(utf8_path).unwrap();
    assert_eq!(loaded.agent_name, "agent1");
    assert_eq!(loaded.runtime_generation, 3);
    assert_eq!(loaded.leader_pid, 42);
    assert_eq!(loaded.started_at, Some("2025-01-01T00:00:00Z".to_string()));
}

#[test]
#[should_panic(expected = "leader_pid must be positive")]
fn test_helper_manifest_zero_pid_panics() {
    ProviderHelperManifest::new("agent1", 1, "codex_bridge", 0);
}

#[test]
fn test_build_codex_helper_manifest() {
    let runtime = RuntimeInfo {
        agent_name: "agent1".to_string(),
        provider: "codex".to_string(),
        runtime_root: Some("/tmp/runtime".to_string()),
        runtime_generation: Some(5),
        started_at: Some("2025-01-01T00:00:00Z".to_string()),
        last_seen_at: None,
        daemon_generation: Some(1),
        state: "busy".to_string(),
    };
    // Without a bridge.pid file the manifest returns None.
    assert!(build_runtime_helper_manifest(&runtime).is_none());
}

#[test]
fn test_provider_health_snapshot_store_tracks_job_history() {
    let dir = TempDir::new().unwrap();
    let layout = PathLayout::new(camino::Utf8Path::from_path(dir.path()).unwrap());
    let store = ProviderHealthSnapshotStore::new(layout);

    let first = ProviderHealthSnapshot::new("job-1", "codex", "Agent1", "2026-03-30T12:00:00Z")
        .with_runtime_alive(true)
        .with_session_reachable(Some(true))
        .with_progress_state(ProgressState::Accepted)
        .with_completion_state(ProviderCompletionState::NotComplete)
        .with_last_progress_at("2026-03-30T12:00:00Z")
        .with_diagnostics(HashMap::from([(
            "phase".to_string(),
            Value::String("accepted".to_string()),
        )]));

    let second = ProviderHealthSnapshot::new("job-1", "codex", "agent1", "2026-03-30T12:00:05Z")
        .with_runtime_alive(true)
        .with_session_reachable(Some(true))
        .with_progress_state(ProgressState::OutputAdvancing)
        .with_completion_state(ProviderCompletionState::TerminalComplete)
        .with_last_progress_at("2026-03-30T12:00:03Z")
        .with_diagnostics(HashMap::from([(
            "phase".to_string(),
            Value::String("complete".to_string()),
        )]));

    store.append(&first).unwrap();
    store.append(&second).unwrap();

    let latest = store.latest("job-1").expect("latest snapshot exists");
    assert_eq!(latest.agent_name, "agent1");
    assert_eq!(latest.progress_state, ProgressState::OutputAdvancing);
    assert_eq!(
        latest.completion_state,
        ProviderCompletionState::TerminalComplete
    );

    assert_eq!(store.list_job("job-1").unwrap().len(), 2);
    assert_eq!(store.list_all().len(), 2);
}

#[test]
fn test_cleanup_stale_runtime_helper_no_manifest() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("helper.json");
    let utf8_path = camino::Utf8Path::from_path(&path).unwrap();
    let runtime = RuntimeInfo {
        agent_name: "agent1".to_string(),
        provider: "codex".to_string(),
        runtime_root: Some("/tmp/runtime".to_string()),
        runtime_generation: Some(1),
        started_at: None,
        last_seen_at: None,
        daemon_generation: None,
        state: "busy".to_string(),
    };
    assert!(!cleanup_stale_runtime_helper(utf8_path, &runtime));
}

/// Wave 3 P1 contract lock: every targeted execution adapter is registered
/// with a non-`None` adapter whose `provider()` matches its canonical name.
/// Mirrors Python `lib/provider_execution/registry.py::build_default_execution_registry`.
#[test]
fn test_execution_registry_has_all_wave3_adapters() {
    let registry = ccb_providers::build_default_execution_registry();
    for provider in ["codex", "claude", "gemini", "droid", "agy", "opencode"] {
        let adapter = registry.get(provider);
        assert!(
            adapter.is_some(),
            "missing execution adapter for {}",
            provider
        );
        assert_eq!(adapter.unwrap().provider(), provider);
    }
}
