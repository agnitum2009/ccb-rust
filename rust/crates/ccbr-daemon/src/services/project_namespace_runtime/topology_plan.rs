//! Mirrors Python `lib/ccbrd/services/project_namespace_runtime/topology_plan.py`.

use ccbr_agents::models::{ProjectConfig, SidebarSpec, ToolWindowSpec, WindowSpec};
use serde::{Deserialize, Serialize};

/// Sidebar pane plan attached to a window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarPanePlan {
    pub mode: String,
    pub width: String,
    pub bottom_height: u32,
    pub launch_args: Vec<String>,
}

impl SidebarPanePlan {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": self.mode,
            "width": self.width,
            "bottom_height": self.bottom_height,
            "launch_args": self.launch_args,
        })
    }
}

/// Plan for a single namespace window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceWindowPlan {
    pub name: String,
    pub order: u32,
    pub kind: String,
    pub label: Option<String>,
    pub command: Option<String>,
    pub user_layout: String,
    pub realized_layout: String,
    pub agent_names: Vec<String>,
    pub sidebar: Option<SidebarPanePlan>,
}

impl NamespaceWindowPlan {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "order": self.order,
            "kind": self.kind,
            "label": self.label,
            "command": self.command,
            "user_layout": self.user_layout,
            "realized_layout": self.realized_layout,
            "agent_names": self.agent_names,
            "sidebar": self.sidebar.as_ref().map(SidebarPanePlan::to_record),
        })
    }
}

/// Topology plan describing how a project config maps onto the namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceTopologyPlan {
    pub signature: String,
    pub entry_window: String,
    pub sidebar_enabled: bool,
    pub windows: Vec<NamespaceWindowPlan>,
    /// CCBRD socket path used for sidebar launch args (not part of the Python record).
    pub ccbrd_socket_path: Option<String>,
    /// Project root used for sidebar launch args (not part of the Python record).
    pub project_root: Option<String>,
}

impl NamespaceTopologyPlan {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "signature": self.signature,
            "entry_window": self.entry_window,
            "sidebar_enabled": self.sidebar_enabled,
            "windows": self.windows.iter().map(NamespaceWindowPlan::to_record).collect::<Vec<_>>(),
        })
    }
}

/// Build a namespace topology plan from a project config.
pub fn build_namespace_topology_plan(
    config: &ProjectConfig,
    ccbrd_socket_path: Option<String>,
    project_root: Option<String>,
) -> NamespaceTopologyPlan {
    let sidebar = config.sidebar.as_ref();
    let sidebar_enabled = sidebar
        .map(|s| s.mode.trim() == ccbr_agents::models::SIDEBAR_MODE_EVERY_WINDOW)
        .unwrap_or(false);
    let sidebar_for_window = sidebar.filter(|_| sidebar_enabled);

    let agent_windows = config.windows.as_deref().unwrap_or_default();
    let tool_windows = config.tool_windows.as_deref().unwrap_or_default();

    let mut windows: Vec<NamespaceWindowPlan> = agent_windows
        .iter()
        .map(|window| {
            window_plan(
                window,
                sidebar_for_window,
                ccbrd_socket_path.as_deref(),
                project_root.as_deref(),
            )
        })
        .collect();

    let order_offset = windows.len() as u32;
    windows.extend(tool_windows.iter().map(|tool| {
        tool_window_plan(
            tool,
            order_offset,
            sidebar_for_window,
            ccbrd_socket_path.as_deref(),
            project_root.as_deref(),
        )
    }));

    NamespaceTopologyPlan {
        signature: config.topology_signature.clone().unwrap_or_default(),
        entry_window: config
            .entry_window
            .clone()
            .or_else(|| agent_windows.first().map(|w| w.name.clone()))
            .unwrap_or_else(|| "main".to_string()),
        sidebar_enabled,
        windows,
        ccbrd_socket_path,
        project_root,
    }
}

fn window_plan(
    window: &WindowSpec,
    sidebar: Option<&SidebarSpec>,
    ccbrd_socket_path: Option<&str>,
    project_root: Option<&str>,
) -> NamespaceWindowPlan {
    let sidebar_plan = sidebar_plan(sidebar, &window.name, ccbrd_socket_path, project_root);
    NamespaceWindowPlan {
        name: window.name.clone(),
        order: window.order,
        kind: "agents".to_string(),
        label: Some(window.name.clone()),
        command: None,
        user_layout: window.layout_spec.clone(),
        realized_layout: realized_layout(&window.layout_spec, sidebar_plan.is_some()),
        agent_names: window.agent_names.clone(),
        sidebar: sidebar_plan,
    }
}

fn tool_window_plan(
    tool: &ToolWindowSpec,
    order_offset: u32,
    sidebar: Option<&SidebarSpec>,
    ccbrd_socket_path: Option<&str>,
    project_root: Option<&str>,
) -> NamespaceWindowPlan {
    let sidebar_plan = sidebar_plan(sidebar, &tool.name, ccbrd_socket_path, project_root);
    NamespaceWindowPlan {
        name: tool.name.clone(),
        order: order_offset + tool.order,
        kind: "tool".to_string(),
        label: tool.label.clone(),
        command: Some(tool.command.clone()),
        user_layout: tool.command.clone(),
        realized_layout: if sidebar_plan.is_some() {
            "sidebar; (tool)".to_string()
        } else {
            "tool".to_string()
        },
        agent_names: Vec::new(),
        sidebar: sidebar_plan,
    }
}

fn sidebar_plan(
    sidebar: Option<&SidebarSpec>,
    window_name: &str,
    ccbrd_socket_path: Option<&str>,
    project_root: Option<&str>,
) -> Option<SidebarPanePlan> {
    sidebar.map(|s| SidebarPanePlan {
        mode: s.mode.clone(),
        width: s.width.as_string(),
        bottom_height: s.bottom_height,
        launch_args: sidebar_launch_args(ccbrd_socket_path, project_root, window_name),
    })
}

fn realized_layout(user_layout: &str, sidebar_enabled: bool) -> String {
    if sidebar_enabled {
        format!("sidebar; ({user_layout})")
    } else {
        user_layout.to_string()
    }
}

fn sidebar_launch_args(
    ccbrd_socket_path: Option<&str>,
    project_root: Option<&str>,
    window_name: &str,
) -> Vec<String> {
    let mut args = vec!["ccbr-agent-sidebar".to_string()];
    if let Some(path) = ccbrd_socket_path {
        args.push("--ccbrd-socket".to_string());
        args.push(path.to_string());
    }
    if let Some(root) = project_root {
        args.push("--project-root".to_string());
        args.push(root.to_string());
    }
    args.push("--pane-window".to_string());
    args.push(window_name.to_string());
    args
}
