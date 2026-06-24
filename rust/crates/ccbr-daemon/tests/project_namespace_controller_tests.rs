use camino::Utf8PathBuf;
use ccbr_agents::models::{
    ProjectConfig, SidebarDimension, SidebarSpec, WindowSpec, SIDEBAR_MODE_EVERY_WINDOW,
};
use ccbr_daemon::services::project_namespace_runtime::{
    controller::ProjectNamespaceController,
    ensure_context::Clock,
    test_support::{FakeTmuxBackend, Pane, Window},
    topology_plan::build_namespace_topology_plan,
};
use ccbr_daemon::services::project_namespace_state::{
    ProjectNamespaceEventStore, ProjectNamespaceState, ProjectNamespaceStateStore,
};
use ccbr_storage::paths::PathLayout;

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

    let namespace = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();

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
        guard.pane_options.get("%2").unwrap().get("@ccbr_slot"),
        Some(&"cmd".to_string())
    );
    assert_eq!(
        guard
            .pane_options
            .get("%2")
            .unwrap()
            .get("@ccbr_namespace_epoch"),
        Some(&"1".to_string())
    );
    assert_eq!(
        guard
            .pane_options
            .get("%2")
            .unwrap()
            .get("@ccbr_managed_by"),
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
    assert_eq!(
        event.details.get("recreated"),
        Some(&serde_json::json!(false))
    );
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

    controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
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
    assert_eq!(
        latest_event.details.get("destroyed"),
        Some(&serde_json::json!(true))
    );
}

fn two_window_sidebar_config() -> ProjectConfig {
    ProjectConfig {
        topology_signature: Some("cmd; agent1:codex".to_string()),
        entry_window: Some("review".to_string()),
        windows: Some(vec![
            WindowSpec {
                name: "main".to_string(),
                order: 0,
                layout_spec: "agent1".to_string(),
                agent_names: vec!["agent1".to_string()],
            },
            WindowSpec {
                name: "review".to_string(),
                order: 1,
                layout_spec: "agent2, agent3".to_string(),
                agent_names: vec!["agent2".to_string(), "agent3".to_string()],
            },
        ]),
        sidebar: Some(SidebarSpec {
            mode: SIDEBAR_MODE_EVERY_WINDOW.into(),
            width: SidebarDimension::Percent("15%".into()),
            bottom_height: 20,
        }),
        ..Default::default()
    }
}

fn topology_plan(
    config: &ProjectConfig,
    layout: &PathLayout,
) -> ccbr_daemon::services::project_namespace_runtime::topology_plan::NamespaceTopologyPlan {
    build_namespace_topology_plan(
        config,
        Some(layout.ccbd_socket_path().to_string()),
        Some(layout.project_root.as_str().to_string()),
    )
}

#[test]
fn test_project_namespace_controller_materializes_explicit_windows_and_sidebar() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology",
        Some(Clock::new(|| "2026-04-03T02:15:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let plan = topology_plan(&two_window_sidebar_config(), &layout);
    let namespace = controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    let state_arc = backend.state();
    let guard = state_arc.lock().unwrap();
    let windows: std::collections::HashMap<String, _> = guard
        .sessions
        .get(&layout.ccbd_tmux_session_name())
        .unwrap()
        .iter()
        .map(|w| (w.name.clone(), w.clone()))
        .collect();

    assert_eq!(
        windows
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>(),
        {
            let mut s = std::collections::HashSet::new();
            s.insert("main".to_string());
            s.insert("review".to_string());
            s
        }
    );
    assert_eq!(namespace.workspace_window_name, Some("review".to_string()));
    assert_eq!(
        guard.active_windows.get(&layout.ccbd_tmux_session_name()),
        Some(&"review".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%1").unwrap().get("@ccbr_role"),
        Some(&"sidebar".to_string())
    );
    assert_eq!(
        guard
            .pane_options
            .get("%1")
            .unwrap()
            .get("@ccbr_sidebar_instance"),
        Some(&"main".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%3").unwrap().get("@ccbr_role"),
        Some(&"sidebar".to_string())
    );
    assert_eq!(
        guard
            .pane_options
            .get("%3")
            .unwrap()
            .get("@ccbr_sidebar_instance"),
        Some(&"review".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%2").unwrap().get("@ccbr_slot"),
        Some(&"agent1".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%4").unwrap().get("@ccbr_slot"),
        Some(&"agent2".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%5").unwrap().get("@ccbr_slot"),
        Some(&"agent3".to_string())
    );
    assert!(guard
        .split_calls
        .contains(&("%1".to_string(), "right".to_string(), 85)));
    assert!(guard
        .split_calls
        .contains(&("%3".to_string(), "right".to_string(), 85)));
    assert_eq!(controller.last_materialized_agent_panes, {
        let mut m = std::collections::HashMap::new();
        m.insert("agent1".to_string(), "%2".to_string());
        m.insert("agent2".to_string(), "%4".to_string());
        m.insert("agent3".to_string(), "%5".to_string());
        m
    });
    let main_key = format!("{}:main", layout.ccbd_tmux_session_name());
    let review_key = format!("{}:review", layout.ccbd_tmux_session_name());
    assert_eq!(
        guard
            .window_options
            .get(&main_key)
            .unwrap()
            .get("pane-border-status"),
        Some(&"top".to_string())
    );
    assert_eq!(
        guard
            .window_options
            .get(&review_key)
            .unwrap()
            .get("pane-border-status"),
        Some(&"top".to_string())
    );
    assert!(guard
        .window_options
        .get(&main_key)
        .unwrap()
        .contains_key("pane-border-format"));
    assert!(guard
        .window_options
        .get(&review_key)
        .unwrap()
        .contains_key("pane-border-format"));
}

fn config_with_windows(windows: Vec<WindowSpec>, sidebar_width: SidebarDimension) -> ProjectConfig {
    ProjectConfig {
        windows: Some(windows),
        sidebar: Some(SidebarSpec {
            mode: SIDEBAR_MODE_EVERY_WINDOW.into(),
            width: sidebar_width,
            bottom_height: 20,
        }),
        ..Default::default()
    }
}

#[test]
fn test_project_namespace_sidebar_width_preserves_agent_grid_area() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology-grid",
        Some(Clock::new(|| "2026-04-03T02:18:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let config = config_with_windows(
        vec![WindowSpec {
            name: "main".to_string(),
            order: 0,
            layout_spec: "agent1:codex, agent2:codex; agent3:codex, agent4:claude".to_string(),
            agent_names: vec![
                "agent1".to_string(),
                "agent2".to_string(),
                "agent3".to_string(),
                "agent4".to_string(),
            ],
        }],
        SidebarDimension::Percent("15%".into()),
    );
    let plan = topology_plan(&config, &layout);
    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    assert_eq!(
        guard.pane_options.get("%1").unwrap().get("@ccbr_role"),
        Some(&"sidebar".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%2").unwrap().get("@ccbr_slot"),
        Some(&"agent1".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%4").unwrap().get("@ccbr_slot"),
        Some(&"agent2".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%3").unwrap().get("@ccbr_slot"),
        Some(&"agent3".to_string())
    );
    assert_eq!(
        guard.pane_options.get("%5").unwrap().get("@ccbr_slot"),
        Some(&"agent4".to_string())
    );
    assert_eq!(
        guard.split_calls,
        vec![
            ("%1".to_string(), "right".to_string(), 85),
            ("%2".to_string(), "right".to_string(), 50),
            ("%2".to_string(), "bottom".to_string(), 50),
            ("%3".to_string(), "bottom".to_string(), 50),
        ]
    );
}

#[test]
fn test_project_namespace_controller_refreshes_topology_ui_for_existing_session() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology-refresh",
        Some(Clock::new(|| "2026-04-03T02:20:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let plan = topology_plan(&two_window_sidebar_config(), &layout);
    let first = controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        let review_key = format!("{}:review", layout.ccbd_tmux_session_name());
        let opts = guard.window_options.entry(review_key).or_default();
        opts.insert("pane-border-status".to_string(), "off".to_string());
        opts.insert(
            "pane-border-format".to_string(),
            "#{pane_index}".to_string(),
        );
    }

    let second = controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    let review_key = format!("{}:review", layout.ccbd_tmux_session_name());
    assert!(!second.created_this_call);
    assert_eq!(second.namespace_epoch, first.namespace_epoch);
    assert_eq!(
        guard
            .window_options
            .get(&review_key)
            .unwrap()
            .get("pane-border-status"),
        Some(&"top".to_string())
    );
    assert_ne!(
        guard
            .window_options
            .get(&review_key)
            .unwrap()
            .get("pane-border-format"),
        Some(&"#{pane_index}".to_string())
    );
}

#[test]
fn test_project_namespace_controller_refreshes_all_sidebar_widths() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology-sidebar-width-refresh",
        Some(Clock::new(|| "2026-04-03T02:22:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let config = config_with_windows(
        vec![
            WindowSpec {
                name: "main".to_string(),
                order: 0,
                layout_spec: "agent1".to_string(),
                agent_names: vec!["agent1".to_string()],
            },
            WindowSpec {
                name: "review".to_string(),
                order: 1,
                layout_spec: "agent2".to_string(),
                agent_names: vec!["agent2".to_string()],
            },
        ],
        SidebarDimension::Percent("15%".into()),
    );
    let plan = topology_plan(&config, &layout);
    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        guard.pane_widths.insert("%1".to_string(), 41);
        guard.pane_widths.insert("%3".to_string(), 23);
        guard.resize_calls.clear();
    }

    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    assert_eq!(
        guard.resize_calls,
        vec![("%1".to_string(), 24), ("%3".to_string(), 24)]
    );
    assert_eq!(guard.pane_widths.get("%1"), Some(&24));
    assert_eq!(guard.pane_widths.get("%3"), Some(&24));
}

#[test]
fn test_project_namespace_controller_preserves_manual_sidebar_width_override() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology-sidebar-width-override",
        Some(Clock::new(|| "2026-04-03T02:22:30Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let config = config_with_windows(
        vec![
            WindowSpec {
                name: "main".to_string(),
                order: 0,
                layout_spec: "agent1".to_string(),
                agent_names: vec!["agent1".to_string()],
            },
            WindowSpec {
                name: "review".to_string(),
                order: 1,
                layout_spec: "agent2".to_string(),
                agent_names: vec!["agent2".to_string()],
            },
        ],
        SidebarDimension::Percent("15%".into()),
    );
    let plan = topology_plan(&config, &layout);
    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        guard.pane_widths.insert("%1".to_string(), 41);
        guard.pane_widths.insert("%3".to_string(), 23);
        guard
            .session_options
            .entry(layout.ccbd_tmux_session_name())
            .or_default()
            .insert("@ccbr_sidebar_width_cells".to_string(), "41".to_string());
        guard.resize_calls.clear();
    }

    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    assert_eq!(guard.resize_calls, vec![("%3".to_string(), 41)]);
    assert_eq!(guard.pane_widths.get("%1"), Some(&41));
    assert_eq!(guard.pane_widths.get("%3"), Some(&41));
}

#[test]
fn test_project_namespace_sidebar_integer_width_uses_columns() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology-sidebar-integer-width",
        Some(Clock::new(|| "2026-04-03T02:23:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let config = config_with_windows(
        vec![WindowSpec {
            name: "main".to_string(),
            order: 0,
            layout_spec: "agent1".to_string(),
            agent_names: vec!["agent1".to_string()],
        }],
        SidebarDimension::Pixels(30),
    );
    let plan = topology_plan(&config, &layout);
    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    assert_eq!(
        guard.split_calls[0],
        ("%1".to_string(), "right".to_string(), 81)
    );
    assert_eq!(guard.pane_widths.get("%1"), Some(&30));
}

#[test]
fn test_project_namespace_controller_clears_topology_panes_when_reusing_without_topology() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-topology-clear",
        Some(Clock::new(|| "2026-04-03T02:25:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let config = config_with_windows(
        vec![
            WindowSpec {
                name: "main".to_string(),
                order: 0,
                layout_spec: "agent1".to_string(),
                agent_names: vec!["agent1".to_string()],
            },
            WindowSpec {
                name: "work".to_string(),
                order: 1,
                layout_spec: "agent2".to_string(),
                agent_names: vec!["agent2".to_string()],
            },
        ],
        SidebarDimension::Percent("15%".into()),
    );
    let plan = topology_plan(&config, &layout);
    controller
        .ensure(None, Some(&plan), false, None, None, None)
        .unwrap();
    assert!(!controller.last_materialized_agent_panes.is_empty());

    let namespace = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();

    assert!(!namespace.created_this_call);
    assert!(controller.last_materialized_agent_panes.is_empty());
    assert!(controller.last_topology_active_panes.is_empty());
}

#[test]
fn test_project_namespace_controller_applies_server_policy_when_reusing_session() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-reuse-policy",
        Some(Clock::new(|| "2026-04-03T02:30:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        guard.tmux_calls.clear();
    }
    let namespace = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    assert!(!namespace.created_this_call);
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "destroy-unattached", "off"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "mouse", "on"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "history-limit", "50000"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "set-clipboard", "on"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "focus-events", "on"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "escape-time", "10"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-window-option", "-g", "mode-keys", "vi"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &[
            "bind-key",
            "-T",
            "copy-mode-vi",
            "v",
            "send-keys",
            "-X",
            "begin-selection"
        ]
    ));
    assert!(!contains_call(
        &guard.tmux_calls,
        &[
            "bind-key",
            "-T",
            "copy-mode-vi",
            "y",
            "send-keys",
            "-X",
            "copy-selection-and-cancel"
        ]
    ));
    assert!(clipboard_bind_call(&guard.tmux_calls, "y"));
    assert!(clipboard_bind_call(&guard.tmux_calls, "MouseDragEnd1Pane"));
    assert!(contains_call(
        &guard.tmux_calls,
        &["bind-key", "h", "select-pane", "-L"]
    ));
}

fn contains_call(calls: &[(Vec<String>, bool)], expected: &[&str]) -> bool {
    let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    calls.iter().any(|(args, _)| args == &expected)
}

fn clipboard_bind_call(calls: &[(Vec<String>, bool)], key: &str) -> bool {
    calls.iter().any(|(args, _)| {
        args.len() >= 8
            && args[0] == "bind-key"
            && args[1] == "-T"
            && args[2] == "copy-mode-vi"
            && args[3] == key
            && args[4] == "send-keys"
            && args[5] == "-X"
            && args[6] == "copy-pipe-and-cancel"
    })
}

#[test]
fn test_project_namespace_controller_recreates_missing_session_with_new_epoch() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-2",
        Some(Clock::new(|| "2026-04-03T03:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let first = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        guard.drop_session(&layout.ccbd_tmux_session_name());
    }
    let second = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let latest_event = event_store.load_latest().unwrap();

    assert_eq!(first.namespace_epoch, 1);
    assert_eq!(second.namespace_epoch, 2);
    assert!(latest_event.is_some());
    let event = latest_event.unwrap();
    assert_eq!(event.event_kind, "namespace_created");
    assert_eq!(event.namespace_epoch, Some(2));
    assert_eq!(
        event.details.get("recreated"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(
        event.details.get("reason"),
        Some(&serde_json::json!("missing_session"))
    );
}

#[test]
fn test_project_namespace_controller_recreates_after_kill_when_has_session_reports_no_server_running(
) {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-2b",
        Some(Clock::new(|| "2026-04-03T03:30:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let first = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    controller.destroy("kill", false).unwrap();
    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        guard.has_session_error = Some(format!(
            "no server running on {}",
            layout.ccbd_tmux_socket_path()
        ));
    }
    let second = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let latest_event = event_store.load_latest().unwrap();

    assert_eq!(first.namespace_epoch, 1);
    assert_eq!(second.namespace_epoch, 2);
    assert!(second.ui_attachable);
    assert!(backend
        .state()
        .lock()
        .unwrap()
        .sessions
        .contains_key(&layout.ccbd_tmux_session_name()));
    assert!(latest_event.is_some());
    let event = latest_event.unwrap();
    assert_eq!(event.event_kind, "namespace_created");
    assert_eq!(event.namespace_epoch, Some(2));
    assert_eq!(
        event.details.get("reason"),
        Some(&serde_json::json!("missing_session"))
    );
}

#[test]
fn test_project_namespace_controller_recreates_session_when_layout_version_changes() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let state_store = ProjectNamespaceStateStore::new(&layout);
    state_store
        .save(&ProjectNamespaceState {
            project_id: "proj-5".to_string(),
            namespace_epoch: 4,
            tmux_socket_path: layout.ccbd_tmux_socket_path().to_string(),
            tmux_session_name: layout.ccbd_tmux_session_name(),
            layout_version: 1,
            layout_signature: Some("cmd; agent1:codex".to_string()),
            control_window_name: None,
            control_window_id: None,
            workspace_window_name: None,
            workspace_window_id: None,
            workspace_epoch: 1,
            ui_attachable: true,
            last_started_at: None,
            last_destroyed_at: None,
            last_destroy_reason: None,
        })
        .unwrap();

    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        let session = layout.ccbd_tmux_session_name();
        let window_name = layout.ccbd_tmux_workspace_window_name();
        guard.sessions.insert(
            session.clone(),
            vec![Window {
                id: "@8".to_string(),
                name: window_name.to_string(),
                width: 160,
                panes: vec!["%8".to_string()],
            }],
        );
        guard
            .active_windows
            .insert(session.clone(), window_name.to_string());
        guard.panes.insert(
            "%8".to_string(),
            Pane {
                id: "%8".to_string(),
                width: 160,
                session,
                window: window_name.to_string(),
                ..Default::default()
            },
        );
        guard.pane_widths.insert("%8".to_string(), 160);
    }

    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-5",
        Some(Clock::new(|| "2026-04-03T06:00:00Z".to_string())),
        Some(backend.backend_factory()),
        Some(state_store),
        None,
        3,
    )
    .unwrap();

    let namespace = controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let latest_event = event_store.load_latest().unwrap();

    assert_eq!(namespace.namespace_epoch, 5);
    assert!(backend.state().lock().unwrap().server_killed);
    assert_eq!(
        backend.state().lock().unwrap().pane_titles.get("%2"),
        Some(&"cmd".to_string())
    );
    assert!(latest_event.is_some());
    assert_eq!(
        latest_event.unwrap().details.get("reason"),
        Some(&serde_json::json!("layout_version_changed"))
    );
}

#[test]
fn test_project_namespace_controller_recreates_session_when_layout_signature_changes() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let state_store = ProjectNamespaceStateStore::new(&layout);
    state_store
        .save(&ProjectNamespaceState {
            project_id: "proj-6".to_string(),
            namespace_epoch: 7,
            tmux_socket_path: layout.ccbd_tmux_socket_path().to_string(),
            tmux_session_name: layout.ccbd_tmux_session_name(),
            layout_version: 3,
            layout_signature: Some("cmd; agent1:codex".to_string()),
            control_window_name: None,
            control_window_id: None,
            workspace_window_name: None,
            workspace_window_id: None,
            workspace_epoch: 1,
            ui_attachable: true,
            last_started_at: None,
            last_destroyed_at: None,
            last_destroy_reason: None,
        })
        .unwrap();

    {
        let state = backend.state();
        let mut guard = state.lock().unwrap();
        let session = layout.ccbd_tmux_session_name();
        let window_name = layout.ccbd_tmux_workspace_window_name();
        guard.sessions.insert(
            session.clone(),
            vec![Window {
                id: "@9".to_string(),
                name: window_name.to_string(),
                width: 160,
                panes: vec!["%9".to_string()],
            }],
        );
        guard
            .active_windows
            .insert(session.clone(), window_name.to_string());
        guard.panes.insert(
            "%9".to_string(),
            Pane {
                id: "%9".to_string(),
                width: 160,
                session,
                window: window_name.to_string(),
                ..Default::default()
            },
        );
        guard.pane_widths.insert("%9".to_string(), 160);
    }

    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-6",
        Some(Clock::new(|| "2026-04-03T07:00:00Z".to_string())),
        Some(backend.backend_factory()),
        Some(state_store),
        None,
        3,
    )
    .unwrap();

    let namespace = controller
        .ensure(
            Some("cmd, agent1:codex; agent2:claude"),
            None,
            false,
            None,
            None,
            None,
        )
        .unwrap();
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let latest_event = event_store.load_latest().unwrap();

    assert_eq!(namespace.namespace_epoch, 8);
    assert_eq!(
        namespace.layout_signature,
        Some("cmd, agent1:codex; agent2:claude".to_string())
    );
    assert!(backend.state().lock().unwrap().server_killed);
    assert_eq!(
        backend.state().lock().unwrap().pane_titles.get("%2"),
        Some(&"cmd".to_string())
    );
    assert!(latest_event.is_some());
    assert_eq!(
        latest_event.unwrap().details.get("reason"),
        Some(&serde_json::json!("layout_signature_changed"))
    );
}

#[test]
fn test_project_namespace_controller_destroy_marks_state_and_event() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-3",
        Some(Clock::new(|| "2026-04-03T04:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    let summary = controller.destroy("kill", false).unwrap();
    let state_store = ProjectNamespaceStateStore::new(&layout);
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let state = state_store.load().unwrap();
    let latest_event = event_store.load_latest().unwrap();

    assert!(summary.destroyed);
    assert_eq!(summary.reason, "kill");
    assert!(backend.state().lock().unwrap().server_killed);
    assert!(state.is_some());
    let state = state.unwrap();
    assert!(!state.ui_attachable);
    assert_eq!(state.last_destroy_reason, Some("kill".to_string()));
    assert!(latest_event.is_some());
    let event = latest_event.unwrap();
    assert_eq!(event.event_kind, "namespace_destroyed");
    assert_eq!(
        event.details.get("reason"),
        Some(&serde_json::json!("kill"))
    );
}

#[test]
fn test_project_namespace_controller_uses_silent_server_commands() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-4",
        Some(Clock::new(|| "2026-04-03T05:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    controller.destroy("kill", false).unwrap();

    let state = backend.state();
    let guard = state.lock().unwrap();
    let new_session_calls: Vec<_> = guard
        .tmux_calls
        .iter()
        .filter(|(args, _)| args.starts_with(&["new-session".to_string(), "-d".to_string()]))
        .collect();
    assert_eq!(new_session_calls.len(), 1);
    let args = &new_session_calls[0].0;
    assert_eq!(
        args[args.len() - 3..],
        [
            "sh".to_string(),
            "-lc".to_string(),
            "while :; do sleep 3600; done".to_string()
        ]
    );
    assert!(contains_call(&guard.tmux_calls, &["start-server"]));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "destroy-unattached", "off"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "mouse", "on"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "history-limit", "50000"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "set-clipboard", "on"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "focus-events", "on"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-option", "-g", "escape-time", "10"]
    ));
    assert!(contains_call(
        &guard.tmux_calls,
        &["set-window-option", "-g", "mode-keys", "vi"]
    ));
    assert!(!contains_call(
        &guard.tmux_calls,
        &[
            "bind-key",
            "-T",
            "copy-mode-vi",
            "y",
            "send-keys",
            "-X",
            "copy-selection-and-cancel"
        ]
    ));
    assert!(clipboard_bind_call(&guard.tmux_calls, "y"));
    assert!(clipboard_bind_call(&guard.tmux_calls, "Enter"));
    assert!(contains_call(
        &guard.tmux_calls,
        &["bind-key", "-r", "L", "resize-pane", "-R", "5"]
    ));
    assert!(contains_call(&guard.tmux_calls, &["kill-server"]));
}

#[test]
fn test_project_namespace_controller_reflow_workspace_persists_epoch_and_event() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-reflow",
        Some(Clock::new(|| "2026-04-03T06:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    controller
        .ensure(None, None, false, None, None, None)
        .unwrap();
    let before_state = ProjectNamespaceStateStore::new(&layout)
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(before_state.workspace_epoch, 1);

    let namespace = controller
        .reflow_workspace(None, Some("reload"), None)
        .unwrap();

    let state_store = ProjectNamespaceStateStore::new(&layout);
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let state = state_store.load().unwrap().unwrap();
    let latest_event = event_store.load_latest().unwrap().unwrap();

    assert!(!namespace.created_this_call);
    assert!(namespace.workspace_recreated_this_call);
    assert_eq!(namespace.workspace_epoch, 2);
    assert_eq!(state.workspace_epoch, 2);
    assert_eq!(state.namespace_epoch, before_state.namespace_epoch);
    assert_eq!(latest_event.event_kind, "workspace_reflowed");
    assert_eq!(
        latest_event.details.get("reason"),
        Some(&serde_json::json!("reload"))
    );
}

#[test]
fn test_project_namespace_controller_waits_for_delayed_window_and_pane_visibility() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    backend.set_window_visibility_lag(1);
    backend.set_pane_visibility_lag(1);
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-delay-1",
        Some(Clock::new(|| "2026-04-03T07:30:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let namespace = controller
        .ensure(None, None, false, None, Some(0.5), None)
        .unwrap();

    let state_store = ProjectNamespaceStateStore::new(&layout);
    let state = state_store.load().unwrap();

    assert_eq!(
        namespace.workspace_window_name,
        Some(layout.ccbd_tmux_workspace_window_name().to_string())
    );
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(
        state.workspace_window_name,
        Some(layout.ccbd_tmux_workspace_window_name().to_string())
    );
    let state = backend.state();
    let guard = state.lock().unwrap();
    let has_cmd_pane = guard.pane_titles.values().any(|t| t == "cmd");
    assert!(
        has_cmd_pane,
        "expected a pane titled 'cmd', got {:?}",
        guard.pane_titles
    );
}

#[test]
fn test_project_namespace_controller_reflow_waits_for_delayed_workspace_visibility() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-reflow-delay",
        Some(Clock::new(|| "2026-04-03T08:30:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    controller
        .ensure(None, None, false, None, Some(0.5), None)
        .unwrap();
    backend.set_window_visibility_lag(2);

    let namespace = controller
        .reflow_workspace(None, Some("pane_recovery:agent1"), Some(0.5))
        .unwrap();

    assert_eq!(namespace.workspace_epoch, 2);
    assert!(!backend.state().lock().unwrap().server_killed);
    assert_eq!(
        backend
            .state()
            .lock()
            .unwrap()
            .active_windows
            .get(&layout.ccbd_tmux_session_name()),
        Some(&layout.ccbd_tmux_workspace_window_name().to_string())
    );
}

#[test]
fn test_project_namespace_controller_reflow_workspace_fresh_namespace_falls_back_to_ensure() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-reflow-fallback",
        Some(Clock::new(|| "2026-04-03T08:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let namespace = controller
        .reflow_workspace(None, Some("reload"), Some(0.5))
        .unwrap();

    let state_store = ProjectNamespaceStateStore::new(&layout);
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let state = state_store.load().unwrap().unwrap();
    let latest_event = event_store.load_latest().unwrap().unwrap();

    assert!(namespace.created_this_call);
    assert!(!namespace.workspace_recreated_this_call);
    assert_eq!(namespace.namespace_epoch, 1);
    assert_eq!(state.workspace_epoch, 1);
    assert_eq!(latest_event.event_kind, "namespace_created");
    assert_eq!(
        latest_event.details.get("reason"),
        Some(&serde_json::json!("reload"))
    );
}

#[test]
fn test_project_namespace_controller_reflow_workspace_replaces_window_without_killing_server() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-reflow-replace",
        Some(Clock::new(|| "2026-04-03T08:15:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let first = controller
        .ensure(None, None, false, None, Some(0.5), None)
        .unwrap();
    let namespace = controller
        .reflow_workspace(None, Some("pane_recovery:agent1"), Some(0.5))
        .unwrap();

    let state_store = ProjectNamespaceStateStore::new(&layout);
    let event_store = ProjectNamespaceEventStore::new(&layout);
    let state = state_store.load().unwrap().unwrap();
    let latest_event = event_store.load_latest().unwrap().unwrap();

    assert_eq!(first.namespace_epoch, 1);
    assert_eq!(namespace.namespace_epoch, 1);
    assert!(!namespace.created_this_call);
    assert!(namespace.workspace_recreated_this_call);
    assert_eq!(namespace.workspace_epoch, 2);
    assert_eq!(state.workspace_epoch, 2);
    assert_eq!(state.control_window_id, Some("@1".to_string()));
    assert_eq!(state.workspace_window_id, Some("@3".to_string()));
    assert!(!backend.state().lock().unwrap().server_killed);
    assert_eq!(
        backend
            .state()
            .lock()
            .unwrap()
            .active_windows
            .get(&layout.ccbd_tmux_session_name()),
        Some(&layout.ccbd_tmux_workspace_window_name().to_string())
    );
    assert_eq!(
        backend.state().lock().unwrap().pane_titles.get("%3"),
        Some(&"cmd".to_string())
    );
    assert_eq!(latest_event.event_kind, "workspace_reflowed");
    assert_eq!(
        latest_event.details.get("reason"),
        Some(&serde_json::json!("pane_recovery:agent1"))
    );

    // Targeted post-reflow commands should use window ids, never the transient name.
    let targeted: Vec<_> = backend
        .state()
        .lock()
        .unwrap()
        .tmux_calls
        .iter()
        .filter(|(args, _)| {
            matches!(
                args.first().map(|s| s.as_str()),
                Some("select-window") | Some("rename-window") | Some("kill-window")
            )
        })
        .map(|(args, _)| args.clone())
        .collect();
    assert!(!targeted.is_empty());
    for args in &targeted {
        let target = args.get(2).expect("target argument");
        assert!(!target.contains(".__reflow__."));
        assert!(target.starts_with(&format!("{}:@", layout.ccbd_tmux_session_name())));
    }
}

#[test]
fn test_prepare_server_failure_includes_diagnostics() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    backend.fail_start_server(
        "error connecting to /private/tmp/tmux-501/default (No such file or directory)",
    );
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-prepare-fail",
        Some(Clock::new(|| "2026-04-03T09:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let result = controller.ensure(None, None, false, None, Some(0.2), None);

    assert!(result.is_err());
    let err = result.unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("failed to prepare tmux server"),
        "expected failure context, got: {text}"
    );
    assert!(
        text.contains(&layout.ccbd_tmux_socket_path().to_string()),
        "expected socket path, got: {text}"
    );
    assert!(
        text.contains("tmux_command="),
        "expected command context, got: {text}"
    );
    assert!(
        text.contains("No such file or directory"),
        "expected stderr detail, got: {text}"
    );
}
