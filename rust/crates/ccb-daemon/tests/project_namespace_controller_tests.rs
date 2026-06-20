use camino::Utf8PathBuf;
use ccb_daemon::services::project_namespace_runtime::{
    controller::ProjectNamespaceController,
    ensure_context::Clock,
    test_support::FakeTmuxBackend,
};
use ccb_daemon::services::project_namespace_state::{
    ProjectNamespaceEventStore, ProjectNamespaceStateStore,
};
use ccb_storage::paths::PathLayout;

fn tmp_layout() -> (PathLayout, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    let layout = PathLayout::new(Utf8PathBuf::from_path_buf(root).unwrap());
    (layout, tmp)
}

#[test]
fn test_project_namespace_controller_creates_state_and_lifecycle_event() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-1",
        Some(Clock::new(|| "2026-04-03T02:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let namespace = controller.ensure(None, None, false, None, None, None).unwrap();

    let state_store = ProjectNamespaceStateStore::new(&layout);
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let state = state_store.load().unwrap();
    let latest_event = event_store.load_latest().unwrap();
    let state_arc = backend.state();
    let guard = state_arc.lock().unwrap();

    assert_eq!(namespace.project_id, "proj-1");
    assert_eq!(namespace.namespace_epoch, 1);
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(
        state.tmux_socket_path,
        layout.ccbd_tmux_socket_path().to_string()
    );
    assert_eq!(state.tmux_session_name, layout.ccbd_tmux_session_name());
    assert_eq!(
        state.control_window_name,
        Some(layout.ccbd_tmux_control_window_name().to_string())
    );
    assert_eq!(
        state.workspace_window_name,
        Some(layout.ccbd_tmux_workspace_window_name().to_string())
    );
    assert_eq!(state.workspace_epoch, 1);
    assert_eq!(
        guard.active_windows.get(&layout.ccbd_tmux_session_name()),
        Some(&layout.ccbd_tmux_workspace_window_name().to_string())
    );
    assert_eq!(guard.pane_titles.get("%2"), Some(&"cmd".to_string()));
    assert_eq!(
        guard.pane_options.get("%2").unwrap().get("@ccb_slot"),
        Some(&"cmd".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%2").unwrap().get("@ccb_namespace_epoch"),
        Some(&"1".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%2").unwrap().get("@ccb_managed_by"),
        Some(&"ccbd".to_string())
    );
    let window_key = format!(
        "{}:{}",
        layout.ccbd_tmux_session_name(),
        layout.ccbd_tmux_workspace_window_name()
    );
    assert_eq!(
        guard
            .window_options
            .get(&window_key)
            .unwrap()
            .get("pane-border-status"),
        Some(&"top".to_string())
    );
    assert!(guard
        .hooks
        .get(&layout.ccbd_tmux_session_name())
        .unwrap()
        .contains_key("after-select-pane"));
    assert!(latest_event.is_some());
    let event = latest_event.unwrap();
    assert_eq!(event.event_kind, "namespace_created");
    assert_eq!(event.details.get("recreated"), Some(&serde_json::json!(false)));
    assert_eq!(
        event.details.get("reason"),
        Some(&serde_json::json!("initial_create"))
    );
}

#[test]
fn test_project_namespace_controller_destroy_persists_destroyed_state() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-1",
        Some(Clock::new(|| "2026-04-03T02:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    controller.ensure(None, None, false, None, None, None).unwrap();
    let summary = controller.destroy("kill", false).unwrap();

    let state_store = ProjectNamespaceStateStore::new(&layout);
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let state = state_store.load().unwrap().unwrap();
    let latest_event = event_store.load_latest().unwrap().unwrap();

    assert!(summary.destroyed);
    assert_eq!(summary.reason, "kill");
    assert_eq!(state.project_id, "proj-1");
    assert!(!state.ui_attachable);
    assert_eq!(state.last_destroy_reason, Some("kill".to_string()));
    assert_eq!(latest_event.event_kind, "namespace_destroyed");
    assert_eq!(latest_event.details.get("destroyed"), Some(&serde_json::json!(true)));
}
