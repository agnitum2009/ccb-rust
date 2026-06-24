use ccbr_agents::models::{
    ProjectConfig, SidebarDimension, SidebarSpec, ToolWindowSpec, WindowSpec,
};
use ccbr_daemon::services::project_namespace_runtime::topology_plan::build_namespace_topology_plan;

fn test_config_with_sidebar() -> ProjectConfig {
    ProjectConfig {
        topology_signature: Some("cmd; agent1:codex".to_string()),
        entry_window: Some("review".to_string()),
        windows: Some(vec![WindowSpec {
            name: "main".to_string(),
            order: 0,
            layout_spec: "agent1".to_string(),
            agent_names: vec!["agent1".to_string()],
        }]),
        tool_windows: Some(vec![ToolWindowSpec {
            name: "logs".to_string(),
            order: 1,
            command: "tail -f /tmp/log".to_string(),
            label: Some("Logs".to_string()),
            show_in_sidebar: true,
        }]),
        sidebar: Some(SidebarSpec {
            mode: ccbr_agents::models::SIDEBAR_MODE_EVERY_WINDOW.into(),
            width: SidebarDimension::Percent("20%".into()),
            bottom_height: 25,
        }),
        ..Default::default()
    }
}

#[test]
fn test_build_topology_plan_with_sidebar() {
    let config = test_config_with_sidebar();
    let plan = build_namespace_topology_plan(
        &config,
        Some("/tmp/ccbr.sock".to_string()),
        Some("/tmp/repo".to_string()),
    );

    assert_eq!(plan.signature, "cmd; agent1:codex");
    assert_eq!(plan.entry_window, "review");
    assert!(plan.sidebar_enabled);
    assert_eq!(plan.windows.len(), 2);

    let main = &plan.windows[0];
    assert_eq!(main.name, "main");
    assert_eq!(main.order, 0);
    assert_eq!(main.kind, "agents");
    assert_eq!(main.label.as_deref(), Some("main"));
    assert_eq!(main.command, None);
    assert_eq!(main.user_layout, "agent1");
    assert_eq!(main.realized_layout, "sidebar; (agent1)");
    assert_eq!(main.agent_names, vec!["agent1"]);

    let sidebar = main.sidebar.as_ref().expect("main should have a sidebar");
    assert_eq!(sidebar.mode, "every_window");
    assert_eq!(sidebar.width, "20%");
    assert_eq!(sidebar.bottom_height, 25);
    assert_eq!(
        sidebar.launch_args,
        vec![
            "ccbr-agent-sidebar",
            "--ccbrd-socket",
            "/tmp/ccbr.sock",
            "--project-root",
            "/tmp/repo",
            "--pane-window",
            "main",
        ]
    );

    let logs = &plan.windows[1];
    assert_eq!(logs.name, "logs");
    assert_eq!(logs.order, 2);
    assert_eq!(logs.kind, "tool");
    assert_eq!(logs.label.as_deref(), Some("Logs"));
    assert_eq!(logs.command.as_deref(), Some("tail -f /tmp/log"));
    assert_eq!(logs.user_layout, "tail -f /tmp/log");
    assert_eq!(logs.realized_layout, "sidebar; (tool)");
    assert!(logs.agent_names.is_empty());
    assert!(logs.sidebar.is_some());
}

#[test]
fn test_build_topology_plan_without_sidebar() {
    let mut config = test_config_with_sidebar();
    config.sidebar = Some(SidebarSpec {
        mode: ccbr_agents::models::SIDEBAR_MODE_OFF.into(),
        width: SidebarDimension::Percent("15%".into()),
        bottom_height: 20,
    });

    let plan = build_namespace_topology_plan(&config, None, None);

    assert!(!plan.sidebar_enabled);
    assert!(plan.windows.iter().all(|w| w.sidebar.is_none()));
    assert_eq!(plan.windows[0].realized_layout, "agent1");
    assert_eq!(plan.windows[1].realized_layout, "tool");
}

#[test]
fn test_build_topology_plan_entry_window_defaults_to_first_window() {
    let mut config = test_config_with_sidebar();
    config.entry_window = None;

    let plan = build_namespace_topology_plan(&config, None, None);
    assert_eq!(plan.entry_window, "main");
}

#[test]
fn test_build_topology_plan_tool_window_order_offset() {
    let config = ProjectConfig {
        windows: Some(vec![
            WindowSpec {
                name: "a".to_string(),
                order: 0,
                layout_spec: "agent1".to_string(),
                agent_names: vec!["agent1".to_string()],
            },
            WindowSpec {
                name: "b".to_string(),
                order: 1,
                layout_spec: "agent2".to_string(),
                agent_names: vec!["agent2".to_string()],
            },
        ]),
        tool_windows: Some(vec![ToolWindowSpec {
            name: "tool".to_string(),
            order: 0,
            command: "cmd".to_string(),
            label: None,
            show_in_sidebar: false,
        }]),
        ..Default::default()
    };

    let plan = build_namespace_topology_plan(&config, None, None);
    assert_eq!(plan.windows.len(), 3);
    assert_eq!(plan.windows[2].name, "tool");
    assert_eq!(plan.windows[2].order, 2);
}
