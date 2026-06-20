//! Mirrors Python `test_v2_agent_store.py` for agent spec/restore persistence.

use ccb_agents::models::{
    AgentRestoreState, AgentSpec, PermissionMode, ProviderProfileSpec, QueuePolicy, RestoreMode,
    RuntimeMode, WorkspaceMode,
};
use ccb_agents::store::{AgentRestoreStore, AgentSpecStore};
use ccb_storage::paths::PathLayout;

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
