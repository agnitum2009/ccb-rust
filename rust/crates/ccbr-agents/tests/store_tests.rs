//! Mirrors Python `test_v2_agent_store.py` for agent spec/restore persistence.

use ccbr_agents::models::{
    AgentApiSpec, AgentRestoreState, AgentRuntime, AgentSpec, AgentState, PermissionMode,
    ProviderProfileSpec, QueuePolicy, RestoreMode, RestoreStatus, RuntimeBindingSource,
    RuntimeMode, WorkspaceMode,
};
use ccbr_agents::store::{AgentRestoreStore, AgentRuntimeStore, AgentSpecStore};
use ccbr_storage::paths::PathLayout;

fn sample_spec(name: &str) -> AgentSpec {
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
fn test_agent_spec_store_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = PathLayout::new(camino::Utf8Path::from_path(tmp.path()).unwrap());
    let store = AgentSpecStore::new(layout);
    let spec = sample_spec("agent1");

    store.save(&spec).unwrap();
    let loaded = store.load("agent1").unwrap().unwrap();
    assert_eq!(loaded.name, "agent1");
    assert_eq!(loaded.provider, "claude");

    assert!(store.remove("agent1").unwrap());
    assert!(store.load("agent1").unwrap().is_none());
}

#[test]
fn test_agent_restore_store_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = PathLayout::new(camino::Utf8Path::from_path(tmp.path()).unwrap());
    let store = AgentRestoreStore::new(layout);
    let state = AgentRestoreState {
        restore_mode: RestoreMode::Provider,
        last_checkpoint: Some("checkpoint-1".into()),
        conversation_summary: "summary".into(),
        open_tasks: vec!["task1".into()],
        files_touched: vec!["file1.rs".into()],
        base_commit: Some("abc123".into()),
        head_commit: Some("def456".into()),
        last_restore_status: None,
    };

    store.save("agent1", &state).unwrap();
    let loaded = store.load("agent1").unwrap().unwrap();
    assert_eq!(loaded.restore_mode, RestoreMode::Provider);
    assert_eq!(loaded.last_checkpoint, Some("checkpoint-1".into()));
    assert_eq!(loaded.open_tasks, vec!["task1"]);

    assert!(store.remove("agent1").unwrap());
    assert!(store.load("agent1").unwrap().is_none());
}

#[test]
fn test_agent_runtime_store_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = PathLayout::new(camino::Utf8Path::from_path(tmp.path()).unwrap());
    let store = AgentRuntimeStore::new(layout);
    let runtime = AgentRuntime {
        agent_name: "agent1".into(),
        state: AgentState::Idle,
        pid: Some(1234),
        project_id: "proj1".into(),
        backend_type: "tmux".into(),
        health: "healthy".into(),
        queue_depth: 0,
        ..Default::default()
    };

    store.save(&runtime).unwrap();
    let loaded = store.load("agent1").unwrap().unwrap();
    assert_eq!(loaded.agent_name, "agent1");
    assert_eq!(loaded.state, AgentState::Idle);
    assert_eq!(loaded.pid, Some(1234));

    assert!(store.remove("agent1").unwrap());
    assert!(store.load("agent1").unwrap().is_none());
}

/// Mirrors Python `test_v2_agent_store.py::test_agent_stores_roundtrip` with all fields populated.
#[test]
fn test_agent_stores_full_field_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = PathLayout::new(camino::Utf8Path::from_path(tmp.path()).unwrap());
    let spec_store = AgentSpecStore::new(layout.clone());
    let runtime_store = AgentRuntimeStore::new(layout.clone());
    let restore_store = AgentRestoreStore::new(layout);

    let spec = AgentSpec {
        name: "agent1".into(),
        provider: "codex".into(),
        target: ".".into(),
        workspace_mode: WorkspaceMode::GitWorktree,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
        restore_default: RestoreMode::Auto,
        permission_default: PermissionMode::Manual,
        queue_policy: QueuePolicy::SerialPerAgent,
        model: Some("gpt-5".into()),
        api: AgentApiSpec {
            key: Some("sk-store".into()),
            url: Some("https://api.store.example.test/v1".into()),
        },
        branch_template: Some("ccb/{agent_name}".into()),
        ..sample_spec("agent1")
    };

    let runtime = AgentRuntime {
        agent_name: "agent1".into(),
        state: AgentState::Idle,
        pid: Some(123),
        started_at: Some("2026-03-18T00:00:00Z".into()),
        last_seen_at: Some("2026-03-18T00:00:01Z".into()),
        runtime_ref: Some("runtime-1".into()),
        session_ref: Some("session-1".into()),
        workspace_path: Some("/workspace/agent1".into()),
        project_id: "proj-1".into(),
        backend_type: "tmux".into(),
        queue_depth: 0,
        socket_path: Some("/sock/ccbd.sock".into()),
        health: "healthy".into(),
        tmux_window_name: Some("main".into()),
        tmux_window_id: Some("@1".into()),
        binding_source: RuntimeBindingSource::ExternalAttach,
        daemon_generation: Some(3),
        desired_state: Some("mounted".into()),
        reconcile_state: Some("steady".into()),
        restart_count: 2,
        last_reconcile_at: Some("2026-03-18T00:00:02Z".into()),
        last_failure_reason: Some("pane-dead".into()),
        ..Default::default()
    };

    let restore = AgentRestoreState {
        restore_mode: RestoreMode::Auto,
        last_checkpoint: Some("checkpoint.md".into()),
        conversation_summary: "summary".into(),
        open_tasks: vec!["task1".into()],
        files_touched: vec!["a.py".into()],
        base_commit: Some("abc".into()),
        head_commit: Some("def".into()),
        last_restore_status: Some(RestoreStatus::Provider),
    };

    spec_store.save(&spec).unwrap();
    runtime_store.save(&runtime).unwrap();
    restore_store.save("agent1", &restore).unwrap();

    let loaded_spec = spec_store.load("agent1").unwrap().unwrap();
    assert_eq!(loaded_spec, spec);

    let loaded_runtime = runtime_store.load("agent1").unwrap().unwrap();
    assert_eq!(loaded_runtime, runtime);

    let loaded_restore = restore_store.load("agent1").unwrap().unwrap();
    assert_eq!(
        loaded_restore.last_restore_status,
        Some(RestoreStatus::Provider)
    );
    assert_eq!(loaded_restore.files_touched, vec!["a.py"]);
}
