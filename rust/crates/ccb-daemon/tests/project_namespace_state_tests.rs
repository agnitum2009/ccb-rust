use std::collections::HashMap;

use ccb_daemon::services::project_namespace_state::{
    next_namespace_epoch, ProjectNamespaceEvent, ProjectNamespaceEventStore, ProjectNamespaceState,
    ProjectNamespaceStateStore,
};
use ccb_storage::paths::PathLayout;

fn tmp_layout() -> (PathLayout, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    let layout = PathLayout::new(utf8_path(&root));
    (layout, tmp)
}

fn utf8_path(p: &std::path::Path) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from_path_buf(p.to_path_buf()).unwrap()
}

#[test]
fn test_project_namespace_state_store_round_trip() {
    let (layout, _tmp) = tmp_layout();
    let state = ProjectNamespaceState::new(
        "proj-1",
        3,
        layout.ccbd_tmux_socket_path().as_str(),
        &layout.ccbd_tmux_session_name(),
    )
    .unwrap()
    .with_layout_version(3)
    .with_layout_signature("cmd; agent1:codex")
    .with_control_window("__ccb_ctl", "@1")
    .with_workspace_window("ccb", "@2")
    .with_workspace_epoch(4)
    .with_started("2026-04-03T01:00:00Z", true)
    .with_destroyed("2026-04-03T00:55:00Z", "kill");

    let store = ProjectNamespaceStateStore::new(&layout);
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().expect("state should be loadable");

    assert_eq!(loaded, state);
    let summary = loaded.summary_fields();
    assert_eq!(
        summary.get("namespace_tmux_socket_path"),
        Some(&serde_json::Value::String(
            layout.ccbd_tmux_socket_path().to_string()
        ))
    );
}

#[test]
fn test_path_layout_normalizes_tmux_session_name_for_tmux_targets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join("repo.with.dots");
    std::fs::create_dir_all(&root).unwrap();
    let layout = PathLayout::new(utf8_path(&root));

    let session_name = layout.ccbd_tmux_session_name();
    assert!(session_name.starts_with("ccb-"));
    assert!(!session_name.contains('.'));
}

#[test]
fn test_event_store_append_and_read_all() {
    let (layout, _tmp) = tmp_layout();
    let store = ProjectNamespaceEventStore::new(&layout);

    let e1 = ProjectNamespaceEvent::new("started", "proj-1", "2026-04-03T01:00:00Z").unwrap();
    let e2 = ProjectNamespaceEvent::new("destroyed", "proj-1", "2026-04-03T01:05:00Z")
        .unwrap()
        .with_namespace_epoch(3)
        .with_socket_path(layout.ccbd_tmux_socket_path().as_str())
        .with_session_name(&layout.ccbd_tmux_session_name())
        .with_details({
            let mut m = HashMap::new();
            m.insert("reason".to_string(), serde_json::json!("kill"));
            m
        });

    store.append(&e1).unwrap();
    store.append(&e2).unwrap();

    let all = store.read_all().unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0], e1);
    assert_eq!(all[1], e2);

    let latest = store.load_latest().unwrap();
    assert_eq!(latest, Some(e2));
}

#[test]
fn test_next_namespace_epoch() {
    assert_eq!(next_namespace_epoch(None), 1);
    let state = ProjectNamespaceState::new("proj-1", 5, "/tmp/x.sock", "ccb-x").unwrap();
    assert_eq!(next_namespace_epoch(Some(&state)), 6);
}
