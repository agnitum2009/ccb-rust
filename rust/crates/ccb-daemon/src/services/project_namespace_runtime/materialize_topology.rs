//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/materialize_topology.py`.
//!
//! Plans, creates, and reconciles tmux windows/panes for a project namespace
//! topology. Pure planning helpers are unit-tested; operations that need a live
//! tmux server are marked `#[ignore]`.

use std::collections::HashMap;

#[allow(unused_imports)]
use ccb_agents::layout::{parse_layout_spec, LayoutLeaf, LayoutNode};
use ccb_terminal::identity::apply_ccb_pane_identity;
use ccb_terminal::placeholders::pane_placeholder_cmd;
use ccb_terminal::theme::render_tmux_session_theme;

use super::backend::{
    create_session, ensure_server_policy, ensure_window, find_window, prepare_server,
    rename_window, select_window, session_window_target, split_pane, window_root_pane, Backend,
    TmuxWindowRecord,
};
use super::ensure_context::{
    NamespaceController, NamespaceEnsureContext, NamespaceWindowPlan, TopologyPlan,
};
use super::sidebar_helper::sidebar_respawn_args;
use crate::{DaemonError, Result};

/// Input bundle for `materialize_topology`.
#[derive(Debug, Clone, Copy)]
pub struct MaterializeTopologyRequest<'a> {
    pub controller: &'a NamespaceController,
    pub context: &'a NamespaceEnsureContext,
    pub topology_plan: &'a TopologyPlan,
    pub epoch: i64,
    pub terminal_size: Option<(i32, i32)>,
    pub timeout_s: Option<f64>,
}

impl<'a> MaterializeTopologyRequest<'a> {
    pub fn execute(self) -> Result<HashMap<String, String>> {
        materialize_topology(
            self.controller,
            self.context,
            self.topology_plan,
            self.epoch,
            self.terminal_size,
            self.timeout_s,
        )
    }
}

/// Refresh UI for an already-materialized topology.
pub fn refresh_topology_ui(context: &NamespaceEnsureContext) -> Result<()> {
    apply_project_tmux_ui(
        &context.backend,
        &context.desired_socket_path,
        None,
        &context.desired_session_name,
    )?;
    _sync_topology_sidebar_widths(None, context, context.topology_plan.as_ref(), None);
    Ok(())
}

/// Refresh UI and sidebar widths for an active project namespace.
pub fn refresh_topology_ui_for_project(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    topology_plan: &TopologyPlan,
    timeout_s: Option<f64>,
) -> Result<()> {
    apply_project_tmux_ui(
        &context.backend,
        &context.desired_socket_path,
        Some(&controller.layout.ccbd_socket_path),
        &context.desired_session_name,
    )?;
    _sync_topology_sidebar_widths(Some(controller), context, Some(topology_plan), timeout_s);
    Ok(())
}

/// Materialize `topology_plan` into tmux windows and panes.
///
/// Returns a map from agent name to the tmux pane id that hosts it.
pub fn materialize_topology(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    topology_plan: &TopologyPlan,
    epoch: i64,
    terminal_size: Option<(i32, i32)>,
    timeout_s: Option<f64>,
) -> Result<HashMap<String, String>> {
    let windows = &topology_plan.windows;
    if windows.is_empty() {
        return Ok(HashMap::new());
    }

    prepare_server(&context.backend, timeout_s)?;

    let first_window = &windows[0];
    if !context.session_is_alive {
        create_session(
            &context.backend,
            &context.desired_session_name,
            &controller.layout.project_root,
            Some(&first_window.name),
            terminal_size,
            timeout_s,
        )?;
    } else {
        ensure_window(
            &context.backend,
            &context.desired_session_name,
            &first_window.name,
            &controller.layout.project_root,
            false,
            timeout_s,
        )?;
    }

    ensure_server_policy(&context.backend, timeout_s)?;
    apply_project_tmux_ui(
        &context.backend,
        &context.desired_socket_path,
        Some(&controller.layout.ccbd_socket_path),
        &context.desired_session_name,
    )?;

    _rename_legacy_workspace_if_needed(controller, context, &first_window.name, timeout_s);

    let mut agent_panes: HashMap<String, String> = HashMap::new();
    for (index, window) in windows.iter().enumerate() {
        ensure_window(
            &context.backend,
            &context.desired_session_name,
            &window.name,
            &controller.layout.project_root,
            index == 0,
            timeout_s,
        )?;

        let target = session_window_target(&context.desired_session_name, Some(&window.name))?;
        let root_pane = window_root_pane(&context.backend, &target, timeout_s)?;

        let user_root =
            _materialize_sidebar(controller, context, window, &root_pane, epoch, timeout_s)?;

        agent_panes.extend(_materialize_agent_layout(
            controller, context, window, &user_root, epoch, timeout_s,
        )?);

        _materialize_tool_window(controller, context, window, &user_root, epoch, timeout_s)?;
    }

    refresh_topology_ui_for_project(controller, context, topology_plan, timeout_s)?;

    select_window(
        &context.backend,
        &session_window_target(
            &context.desired_session_name,
            Some(&topology_plan.entry_window),
        )?,
    )?;

    Ok(agent_panes)
}

/// Find existing agent panes that match the desired topology.
pub fn existing_topology_agent_panes(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    topology_plan: &TopologyPlan,
) -> HashMap<String, String> {
    let mut agent_panes: HashMap<String, String> = HashMap::new();
    for window in &topology_plan.windows {
        for agent_name in &window.agent_names {
            let mut expected = HashMap::new();
            expected.insert("@ccb_project_id".to_string(), controller.project_id.clone());
            expected.insert("@ccb_role".to_string(), "agent".to_string());
            expected.insert("@ccb_slot".to_string(), agent_name.clone());
            expected.insert("@ccb_window".to_string(), window.name.clone());
            expected.insert("@ccb_managed_by".to_string(), "ccbd".to_string());

            let matches = _list_panes_by_user_options(&context.backend, expected);
            if matches.len() == 1 {
                agent_panes.insert(agent_name.clone(), matches[0].clone());
            }
        }
    }
    agent_panes
}

/// Return the pane ids currently considered active for the topology.
pub fn topology_active_panes(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    topology_plan: &TopologyPlan,
) -> Vec<String> {
    let expected_windows: std::collections::HashSet<String> = topology_plan
        .windows
        .iter()
        .map(|w| w.name.clone())
        .collect();

    let mut panes: Vec<String> = Vec::new();
    for role in ["sidebar", "agent", "tool"] {
        let mut expected = HashMap::new();
        expected.insert("@ccb_project_id".to_string(), controller.project_id.clone());
        expected.insert("@ccb_role".to_string(), role.to_string());
        expected.insert("@ccb_managed_by".to_string(), "ccbd".to_string());

        for pane_id in _list_panes_by_user_options(&context.backend, expected) {
            let window_name = _pane_option(&context.backend, &pane_id, "@ccb_window");
            let sidebar_instance =
                _pane_option(&context.backend, &pane_id, "@ccb_sidebar_instance");
            if expected_windows.contains(&window_name)
                || expected_windows.contains(&sidebar_instance)
            {
                panes.push(pane_id);
            }
        }
    }

    // Deduplicate while preserving order.
    let mut seen = std::collections::HashSet::new();
    panes
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// Determine whether the existing topology needs to be recreated.
///
/// Returns a human-readable reason string or `None` if no recreation is needed.
pub fn topology_recreate_reason(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    topology_plan: &TopologyPlan,
) -> Option<String> {
    if let Some(current) = &context.current {
        let current_workspace = current
            .workspace_window_name
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string();
        if !current_workspace.is_empty()
            && current_workspace != context.desired_workspace_window_name
        {
            return Some("topology_workspace_changed".to_string());
        }
    }

    for window in &topology_plan.windows {
        if _find_window(context, &window.name).is_none() {
            return Some(format!("topology_window_missing:{}", window.name));
        }
    }

    let expected_agents: std::collections::HashSet<String> = topology_plan
        .windows
        .iter()
        .flat_map(|w| w.agent_names.iter().cloned())
        .collect();

    let existing_agents: std::collections::HashSet<String> =
        existing_topology_agent_panes(controller, context, topology_plan)
            .into_keys()
            .collect();

    if existing_agents != expected_agents {
        return Some("topology_agent_panes_changed".to_string());
    }

    if topology_plan.sidebar_enabled {
        for window in &topology_plan.windows {
            let mut expected = HashMap::new();
            expected.insert("@ccb_project_id".to_string(), controller.project_id.clone());
            expected.insert("@ccb_role".to_string(), "sidebar".to_string());
            expected.insert("@ccb_sidebar_instance".to_string(), window.name.clone());
            expected.insert("@ccb_managed_by".to_string(), "ccbd".to_string());

            let matches = _list_panes_by_user_options(&context.backend, expected);
            if matches.len() != 1 {
                return Some("topology_sidebar_panes_changed".to_string());
            }
        }
    }

    let expected_tools: std::collections::HashSet<String> = topology_plan
        .windows
        .iter()
        .filter(|w| w.kind == "tool")
        .map(|w| w.name.clone())
        .collect();

    for window_name in expected_tools {
        let mut expected = HashMap::new();
        expected.insert("@ccb_project_id".to_string(), controller.project_id.clone());
        expected.insert("@ccb_role".to_string(), "tool".to_string());
        expected.insert("@ccb_slot".to_string(), format!("tool:{window_name}"));
        expected.insert("@ccb_window".to_string(), window_name.clone());
        expected.insert("@ccb_managed_by".to_string(), "ccbd".to_string());

        let matches = _list_panes_by_user_options(&context.backend, expected);
        if matches.len() != 1 {
            return Some("topology_tool_panes_changed".to_string());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn _rename_legacy_workspace_if_needed(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    first_window_name: &str,
    timeout_s: Option<f64>,
) {
    let mut legacy_name = controller
        .layout
        .ccbd_tmux_workspace_window_name
        .trim()
        .to_string();
    if let Some(current) = &context.current {
        let current_name = current
            .workspace_window_name
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string();
        if !current_name.is_empty() {
            legacy_name = current_name;
        }
    }

    let first_name = first_window_name.trim().to_string();
    if legacy_name.is_empty() || first_name.is_empty() || legacy_name == first_name {
        return;
    }

    let legacy = find_window(
        &context.backend,
        &context.desired_session_name,
        &legacy_name,
        timeout_s,
    );
    let ensure_target = find_window(
        &context.backend,
        &context.desired_session_name,
        &first_name,
        timeout_s,
    );

    let legacy_record = match legacy {
        Ok(Some(r)) => r,
        _ => return,
    };
    if ensure_target.map(|r| r.is_some()).unwrap_or(false) {
        return;
    }

    let target_id = legacy_record
        .window_id
        .as_deref()
        .unwrap_or(&legacy_name)
        .to_string();
    let target = session_window_target(&context.desired_session_name, Some(&target_id))
        .unwrap_or_else(|_| format!("{}:{}", context.desired_session_name, target_id));
    let _ = rename_window(&context.backend, &target, &first_name, timeout_s);
}

fn _materialize_sidebar(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    window: &NamespaceWindowPlan,
    root_pane: &str,
    epoch: i64,
    timeout_s: Option<f64>,
) -> Result<String> {
    let sidebar = match &window.sidebar {
        Some(s) => s,
        None => return Ok(root_pane.to_string()),
    };

    let pane_width = _pane_width_cells(&context.backend, root_pane);
    let user_root = split_pane(
        &context.backend,
        root_pane,
        "right",
        _user_pane_percent_for_sidebar(&sidebar.width, pane_width),
        &controller.layout.project_root,
        timeout_s,
    )?;

    _respawn_sidebar(
        &context.backend,
        root_pane,
        &sidebar.launch_args,
        &controller.layout.project_root,
    );

    apply_ccb_pane_identity(
        &context.backend,
        root_pane,
        "sidebar",
        "sidebar",
        &controller.project_id,
        None,
        false,
        Some("sidebar"),
        Some(&format!("sidebar:{}", window.name)),
        Some(&window.name),
        Some(&window.name),
        None,
        Some(epoch),
        Some("ccbd"),
    );

    Ok(user_root)
}

fn _materialize_agent_layout(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    window: &NamespaceWindowPlan,
    user_root: &str,
    epoch: i64,
    timeout_s: Option<f64>,
) -> Result<HashMap<String, String>> {
    if window.kind == "tool" {
        return Ok(HashMap::new());
    }

    let layout = parse_layout_spec(&window.user_layout).map_err(|e| {
        DaemonError::Config(format!("invalid layout spec for {}: {e}", window.name))
    })?;

    let agent_names: Vec<String> = window
        .agent_names
        .iter()
        .map(|s| s.trim().to_string())
        .collect();
    let style_index_by_agent: HashMap<String, usize> = agent_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i))
        .collect();

    let mut agent_panes: HashMap<String, String> = HashMap::new();

    let mut assign_leaf = |item: &str, pane_id: &str| {
        if item == "cmd" {
            return;
        }
        agent_panes.insert(item.to_string(), pane_id.to_string());
        apply_ccb_pane_identity(
            &context.backend,
            pane_id,
            item,
            item,
            &controller.project_id,
            style_index_by_agent.get(item).map(|i| *i as i32),
            false,
            Some("agent"),
            Some(item),
            Some(&window.name),
            None,
            None,
            Some(epoch),
            Some("ccbd"),
        );
    };

    _materialize_layout(
        controller,
        context,
        user_root,
        &layout,
        &mut assign_leaf,
        timeout_s,
    )?;

    Ok(agent_panes)
}

fn _materialize_tool_window(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    window: &NamespaceWindowPlan,
    user_root: &str,
    epoch: i64,
    _timeout_s: Option<f64>,
) -> Result<()> {
    if window.kind != "tool" {
        return Ok(());
    }

    let command = window
        .command
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(pane_placeholder_cmd);

    let _ = context.backend._tmux_run(
        &["respawn-pane", "-k", "-t", user_root, "sh", "-lc", &command],
        false,
        true,
    );

    let title = window.label.as_deref().unwrap_or(&window.name).to_string();

    apply_ccb_pane_identity(
        &context.backend,
        user_root,
        &title,
        &title,
        &controller.project_id,
        Some(window.order as i32),
        false,
        Some("tool"),
        Some(&format!("tool:{}", window.name)),
        Some(&window.name),
        None,
        None,
        Some(epoch),
        Some("ccbd"),
    );

    let _ = context.backend._tmux_run(
        &["respawn-pane", "-k", "-t", user_root, "sh", "-lc", &command],
        false,
        true,
    );

    Ok(())
}

fn _get_specified_percent(node: &LayoutNode) -> Option<i32> {
    match node {
        LayoutNode::Leaf { leaf } => leaf.percent.map(|p| p as i32),
        LayoutNode::Horizontal { left, right } | LayoutNode::Vertical { left, right } => {
            for leaf in right.iter_leaves() {
                if leaf.percent.is_some() {
                    return leaf.percent.map(|p| p as i32);
                }
            }
            for leaf in left.iter_leaves() {
                if leaf.percent.is_some() {
                    return leaf.percent.map(|p| p as i32);
                }
            }
            None
        }
    }
}

fn _materialize_layout<F>(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    parent_pane_id: &str,
    node: &LayoutNode,
    assign_leaf: &mut F,
    timeout_s: Option<f64>,
) -> Result<()>
where
    F: FnMut(&str, &str),
{
    match node {
        LayoutNode::Leaf { leaf } => {
            assign_leaf(&leaf.name, parent_pane_id);
            return Ok(());
        }
        LayoutNode::Horizontal { left, right } | LayoutNode::Vertical { left, right } => {
            let right_pct = _get_specified_percent(right);
            let left_pct = _get_specified_percent(left);
            let percent = if let Some(pct) = right_pct {
                pct.clamp(1, 99)
            } else if let Some(pct) = left_pct {
                (100 - pct).clamp(1, 99)
            } else {
                let total = node.leaf_count().max(1);
                let right_count = right.leaf_count().max(1);
                ((right_count * 100) / total).clamp(1, 99) as i32
            };

            let direction = if matches!(node, LayoutNode::Horizontal { .. }) {
                "right"
            } else {
                "bottom"
            };

            let new_pane_id = split_pane(
                &context.backend,
                parent_pane_id,
                direction,
                percent,
                &controller.layout.project_root,
                timeout_s,
            )?;

            _materialize_layout(
                controller,
                context,
                parent_pane_id,
                left,
                &mut *assign_leaf,
                timeout_s,
            )?;
            _materialize_layout(
                controller,
                context,
                &new_pane_id,
                right,
                &mut *assign_leaf,
                timeout_s,
            )?;
        }
    }
    Ok(())
}

fn _find_window(context: &NamespaceEnsureContext, window_name: &str) -> Option<TmuxWindowRecord> {
    find_window(
        &context.backend,
        &context.desired_session_name,
        window_name,
        Some(0.0),
    )
    .ok()
    .flatten()
}

fn _sync_topology_sidebar_widths(
    controller: Option<&NamespaceController>,
    context: &NamespaceEnsureContext,
    topology_plan: Option<&TopologyPlan>,
    timeout_s: Option<f64>,
) {
    let plan = match topology_plan {
        Some(p) if p.sidebar_enabled => p,
        _ => return,
    };

    let width_by_window: HashMap<String, String> = plan
        .windows
        .iter()
        .filter_map(|w| {
            w.sidebar
                .as_ref()
                .map(|s| (w.name.clone(), s.width.clone()))
        })
        .collect();

    if width_by_window.is_empty() {
        return;
    }

    let project_id = controller
        .map(|c| c.project_id.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    let width_override =
        _session_sidebar_width_override(&context.backend, &context.desired_session_name);
    _set_session_sidebar_sync_guard(&context.backend, &context.desired_session_name, true);

    for record in
        _list_sidebar_geometry_records(&context.backend, &context.desired_session_name, &project_id)
    {
        let configured_width = if width_override > 0 {
            Some(width_override.to_string())
        } else {
            width_by_window.get(&record.sidebar_instance).cloned()
        };
        let configured_width = match configured_width {
            Some(w) => w,
            None => continue,
        };

        let window_width = _positive_int(&record.window_width);
        if window_width <= 0 {
            continue;
        }
        let target_width = _sidebar_width_cells(&configured_width, window_width);
        if target_width <= 0 || target_width == _positive_int(&record.pane_width) {
            continue;
        }
        _resize_pane_width(&context.backend, &record.pane_id, target_width, timeout_s);
    }

    _set_session_sidebar_sync_guard(&context.backend, &context.desired_session_name, false);
}

#[derive(Debug, Clone)]
struct SidebarGeometryRecord {
    pane_id: String,
    window_width: String,
    pane_width: String,
    sidebar_instance: String,
}

fn _list_sidebar_geometry_records(
    backend: &Backend,
    session_name: &str,
    project_id: &str,
) -> Vec<SidebarGeometryRecord> {
    let output = match backend._tmux_run(
        &[
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{pane_id}\t#{window_width}\t#{pane_width}\t#{@ccb_project_id}\t#{@ccb_role}\t#{@ccb_sidebar_instance}\t#{@ccb_managed_by}",
        ],
        false,
        true,
    ) {
        Ok(out) if out.success() => out.stdout,
        _ => return Vec::new(),
    };

    let mut records = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 8 {
            continue;
        }
        let pane_session = parts[0].trim();
        let pane_id = parts[1].trim();
        let window_width = parts[2].trim();
        let pane_width = parts[3].trim();
        let pane_project_id = parts[4].trim();
        let role = parts[5].trim();
        let sidebar_instance = parts[6].trim();
        let managed_by = parts[7].trim();

        if pane_session != session_name || role != "sidebar" || managed_by != "ccbd" {
            continue;
        }
        if !project_id.is_empty() && pane_project_id != project_id {
            continue;
        }
        if !pane_id.starts_with('%') || sidebar_instance.is_empty() {
            continue;
        }
        records.push(SidebarGeometryRecord {
            pane_id: pane_id.to_string(),
            window_width: window_width.to_string(),
            pane_width: pane_width.to_string(),
            sidebar_instance: sidebar_instance.to_string(),
        });
    }
    records
}

fn _resize_pane_width(backend: &Backend, pane_id: &str, width: i32, _timeout_s: Option<f64>) {
    let width = width.max(1);
    let _ = backend._tmux_run(
        &["resize-pane", "-t", pane_id, "-x", &width.to_string()],
        false,
        true,
    );
}

fn _sidebar_width_cells(width: &str, window_width: i32) -> i32 {
    let usable_width = window_width.max(1);
    let target = _sidebar_width_target_cells(width, usable_width);
    let min_user_width = if usable_width > 20 { 10 } else { 1 };
    let max_width = (usable_width - min_user_width).max(1);
    target.clamp(1, max_width)
}

fn _sidebar_width_target_cells(width: &str, window_width: i32) -> i32 {
    let text = width.trim();
    if text.ends_with('%') {
        let usable_width = window_width.max(1);
        (usable_width as f64 * (_sidebar_percent(text) as f64 / 100.0)).round() as i32
    } else if let Ok(n) = text.parse::<i32>() {
        n
    } else {
        (window_width.max(1) as f64 * 0.15).round() as i32
    }
}

fn _pane_width_cells(backend: &Backend, pane_id: &str) -> i32 {
    let output = match backend._tmux_run(
        &["display-message", "-p", "-t", pane_id, "#{pane_width}"],
        false,
        true,
    ) {
        Ok(out) if out.success() => out.stdout,
        _ => return 0,
    };
    _positive_int(output.lines().next().unwrap_or(""))
}

fn _session_sidebar_width_override(backend: &Backend, session_name: &str) -> i32 {
    let output = match backend._tmux_run(
        &[
            "show-option",
            "-qv",
            "-t",
            session_name,
            "@ccb_sidebar_width_cells",
        ],
        false,
        true,
    ) {
        Ok(out) if out.success() => out.stdout,
        _ => return 0,
    };
    _positive_int(output.lines().next().unwrap_or(""))
}

fn _set_session_sidebar_sync_guard(backend: &Backend, session_name: &str, enabled: bool) {
    let args = if enabled {
        vec![
            "set-option",
            "-t",
            session_name,
            "@ccb_sidebar_sync_guard",
            "1",
        ]
    } else {
        vec![
            "set-option",
            "-u",
            "-t",
            session_name,
            "@ccb_sidebar_sync_guard",
        ]
    };
    let _ = backend._tmux_run(&args, false, true);
}

fn _positive_int(value: &str) -> i32 {
    value.trim().parse::<i32>().map(|n| n.max(0)).unwrap_or(0)
}

fn _pane_option(backend: &Backend, pane_id: &str, option_name: &str) -> String {
    let fmt = format!("#{{{option_name}}}");
    let output =
        match backend._tmux_run(&["display-message", "-p", "-t", pane_id, &fmt], false, true) {
            Ok(out) if out.success() => out.stdout,
            _ => return String::new(),
        };
    output.lines().next().unwrap_or("").trim().to_string()
}

fn _list_panes_by_user_options(
    backend: &Backend,
    expected: HashMap<String, String>,
) -> Vec<String> {
    if expected.is_empty() {
        return Vec::new();
    }

    let options: Vec<String> = expected.keys().cloned().collect();
    let fmt_parts: Vec<String> = std::iter::once("#{pane_id}".to_string())
        .chain(options.iter().map(|o| format!("#{{{o}}}")))
        .collect();
    let fmt = fmt_parts.join("\t");

    let output = match backend._tmux_run(&["list-panes", "-a", "-F", &fmt], false, true) {
        Ok(out) if out.success() => out.stdout,
        _ => return Vec::new(),
    };

    let mut matches = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != options.len() + 1 {
            continue;
        }
        let pane_id = parts[0].trim();
        if !pane_id.starts_with('%') {
            continue;
        }
        let all_match = options.iter().enumerate().all(|(index, option)| {
            let actual = parts[index + 1].trim();
            let expected_value = expected.get(option).map(|s| s.as_str()).unwrap_or("");
            actual == expected_value
        });
        if all_match {
            matches.push(pane_id.to_string());
        }
    }
    matches
}

fn _sidebar_percent(width: &str) -> i32 {
    let text = width.trim();
    let value = if let Some(stripped) = text.strip_suffix('%') {
        stripped
    } else {
        text
    };
    value.parse::<i32>().map(|v| v.clamp(1, 90)).unwrap_or(15)
}

fn _user_pane_percent_for_sidebar(width: &str, pane_width: i32) -> i32 {
    if pane_width > 0 {
        let sidebar_cells = _sidebar_width_cells(width, pane_width);
        let user_cells = (pane_width - sidebar_cells).max(1);
        ((user_cells * 100) / pane_width).clamp(1, 99)
    } else {
        (100 - _sidebar_percent(width)).clamp(10, 99)
    }
}

fn _respawn_sidebar(backend: &Backend, pane_id: &str, launch_args: &[String], _cwd: &str) {
    let args = sidebar_respawn_args(launch_args, None, None);
    let command = if args.is_empty() {
        pane_placeholder_cmd()
    } else {
        shell_join(&args)
    };

    let _ = backend._tmux_run(
        &["respawn-pane", "-k", "-t", pane_id, "sh", "-lc", &command],
        false,
        true,
    );
}

fn shell_join(parts: &[String]) -> String {
    parts
        .iter()
        .map(|s| shell_quote(s))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn shell_quote(s: &str) -> String {
    if s.is_empty()
        || s.chars().any(|c| {
            c.is_whitespace() || c == '\'' || c == '"' || c == '\\' || c == '$' || c == '`'
        })
    {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    } else {
        s.to_string()
    }
}

pub(crate) fn apply_project_tmux_ui(
    backend: &Backend,
    tmux_socket_path: &str,
    _ccbd_socket_path: Option<&str>,
    tmux_session_name: &str,
) -> Result<()> {
    let theme = render_tmux_session_theme(env!("CARGO_PKG_VERSION"), None, None, None, None);

    for (option, value) in &theme.session_options {
        let _ = backend._tmux_run(
            &["set-option", "-t", tmux_session_name, option, value],
            false,
            true,
        );
    }

    let windows = _list_window_names(backend, tmux_session_name)?;
    for window in windows {
        let target = session_window_target(tmux_session_name, Some(&window))?;
        for (option, value) in &theme.window_options {
            let _ = backend._tmux_run(
                &["set-window-option", "-t", &target, option, value],
                false,
                true,
            );
        }
    }

    let _ = backend._tmux_run(
        &[
            "bind-key",
            "-T",
            "root",
            "MouseDown1Pane",
            "select-pane -t = ; send-keys -M",
        ],
        false,
        true,
    );
    let _ = backend._tmux_run(
        &[
            "bind-key",
            "-T",
            "root",
            "MouseDrag1Border",
            "resize-pane -M",
        ],
        false,
        true,
    );

    let socket = shell_quote(tmux_socket_path);
    let session = shell_quote(tmux_session_name);
    let resize_hook = format!(
        "run-shell -b 'current_session=\"#{{session_name}}\"; [ \"$current_session\" = {session} ] || exit 0; guard=$(tmux -S {socket} show-option -qv -t {session} @ccb_sidebar_sync_guard 2>/dev/null || true); [ \"$guard\" = \"1\" ] && exit 0; ccb __sidebar-resize-sync --tmux-socket {socket} --session {session} --source-pane \"#{{pane_id}}\" --project-id \"#{{@ccb_project_id}}\" >/dev/null 2>&1 || true'"
    );
    let _ = backend._tmux_run(
        &[
            "set-hook",
            "-t",
            tmux_session_name,
            "after-resize-pane",
            &resize_hook,
        ],
        false,
        true,
    );

    let select_hook = format!(
        "run-shell -b 'current_session=\"#{{session_name}}\"; [ \"$current_session\" = {session} ] || exit 0; ccb __active-pane-border-sync --tmux-socket {socket} --session {session} --pane \"#{{pane_id}}\" >/dev/null 2>&1 || true'"
    );
    let _ = backend._tmux_run(
        &[
            "set-hook",
            "-t",
            tmux_session_name,
            "after-select-pane",
            &select_hook,
        ],
        false,
        true,
    );

    let window_hook = format!(
        "run-shell -b 'current_session=\"#{{session_name}}\"; [ \"$current_session\" = {session} ] || exit 0; guard=$(tmux -S {socket} show-option -qv -t {session} @ccb_sidebar_sync_guard 2>/dev/null || true); [ \"$guard\" = \"1\" ] && exit 0; ccb __sidebar-resize-sync --tmux-socket {socket} --session {session} --source-window \"#{{window_id}}\" --project-id \"#{{@ccb_project_id}}\" --from-stored-width >/dev/null 2>&1 || true'"
    );
    let _ = backend._tmux_run(
        &["set-hook", "-g", "window-resized", &window_hook],
        false,
        true,
    );

    Ok(())
}

pub(crate) fn _list_window_names(backend: &Backend, session_name: &str) -> Result<Vec<String>> {
    let output = backend
        ._tmux_run(
            &["list-windows", "-t", session_name, "-F", "#{window_name}"],
            false,
            true,
        )
        .map_err(|e| DaemonError::Config(format!("failed to list windows: {e}")))?;

    if !output.success() {
        return Ok(Vec::new());
    }
    Ok(output
        .stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_layout() -> crate::services::project_namespace_runtime::ensure_context::LayoutConfig {
        crate::services::project_namespace_runtime::ensure_context::LayoutConfig {
            project_root: "/tmp/ccb-topo-test".to_string(),
            ccbd_dir: PathBuf::from("/tmp/ccb-topo-test/.ccb"),
            ccbd_socket_path: "/tmp/ccb-topo-test/.ccb/ccbd.sock".to_string(),
            ccbd_tmux_socket_path: "/tmp/ccb-topo-test/.ccb/tmux.sock".to_string(),
            ccbd_tmux_session_name: "ccb-topo-test".to_string(),
            ccbd_tmux_control_window_name: "control".to_string(),
            ccbd_tmux_workspace_window_name: "workspace".to_string(),
        }
    }

    fn test_controller() -> NamespaceController {
        NamespaceController {
            project_id: "p1".to_string(),
            layout_version: 1,
            layout: test_layout(),
            backend_factory:
                crate::services::project_namespace_runtime::backend::BackendFactory::default(),
            state_store:
                crate::services::project_namespace_runtime::ensure_context::StateStore::default(),
            event_store:
                crate::services::project_namespace_runtime::ensure_context::EventStore::default(),
            clock: crate::services::project_namespace_runtime::ensure_context::Clock::new(|| {
                "2024-01-01T00:00:00Z".to_string()
            }),
            last_materialized_agent_panes: HashMap::new(),
            last_topology_active_panes: Vec::new(),
            session_alive_override: None,
        }
    }

    fn test_window(name: &str, layout: &str, agents: &[&str]) -> NamespaceWindowPlan {
        NamespaceWindowPlan {
            name: name.to_string(),
            order: 0,
            kind: "agents".to_string(),
            label: None,
            command: None,
            user_layout: layout.to_string(),
            agent_names: agents.iter().map(|s| s.to_string()).collect(),
            sidebar: None,
        }
    }

    #[test]
    fn test_get_specified_percent_leaf() {
        let node = LayoutNode::Leaf {
            leaf: LayoutLeaf {
                name: "a".to_string(),
                provider: None,
                workspace_mode: None,
                percent: Some(42),
            },
        };
        assert_eq!(_get_specified_percent(&node), Some(42));
    }

    #[test]
    fn test_get_specified_percent_composite_prefers_right() {
        let left = LayoutNode::Leaf {
            leaf: LayoutLeaf {
                name: "left".to_string(),
                provider: None,
                workspace_mode: None,
                percent: Some(30),
            },
        };
        let right = LayoutNode::Leaf {
            leaf: LayoutLeaf {
                name: "right".to_string(),
                provider: None,
                workspace_mode: None,
                percent: Some(70),
            },
        };
        let node = LayoutNode::Horizontal {
            left: Box::new(left),
            right: Box::new(right),
        };
        assert_eq!(_get_specified_percent(&node), Some(70));
    }

    #[test]
    fn test_sidebar_percent() {
        assert_eq!(_sidebar_percent("15%"), 15);
        assert_eq!(_sidebar_percent("150%"), 90);
        assert_eq!(_sidebar_percent("0%"), 1);
        assert_eq!(_sidebar_percent("garbage"), 15);
    }

    #[test]
    fn test_sidebar_width_target_cells() {
        assert_eq!(_sidebar_width_target_cells("15%", 100), 15);
        assert_eq!(_sidebar_width_target_cells("20", 100), 20);
        assert_eq!(_sidebar_width_target_cells("garbage", 100), 15);
    }

    #[test]
    fn test_sidebar_width_cells_respects_minimum_user_width() {
        assert_eq!(_sidebar_width_cells("90%", 100), 90);
        assert_eq!(_sidebar_width_cells("95%", 100), 90); // leaves 10 for user
        assert_eq!(_sidebar_width_cells("50%", 30), 15);
        assert_eq!(_sidebar_width_cells("50%", 15), 8);
    }

    #[test]
    fn test_user_pane_percent_for_sidebar() {
        assert_eq!(_user_pane_percent_for_sidebar("15%", 100), 85);
        assert_eq!(_user_pane_percent_for_sidebar("50%", 100), 50);
        assert_eq!(_user_pane_percent_for_sidebar("15%", 0), 85);
    }

    #[test]
    fn test_positive_int() {
        assert_eq!(_positive_int("42"), 42);
        assert_eq!(_positive_int("  42  "), 42);
        assert_eq!(_positive_int("-3"), 0);
        assert_eq!(_positive_int("abc"), 0);
        assert_eq!(_positive_int(""), 0);
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("hello"), "hello");
        assert_eq!(shell_quote("hello world"), "'hello world'");
        assert_eq!(shell_quote("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn test_shell_join() {
        let parts = vec!["echo".to_string(), "hello world".to_string()];
        assert_eq!(shell_join(&parts), "echo 'hello world'");
    }

    #[test]
    fn test_request_execute_delegates_to_function() {
        let controller = test_controller();
        let context = NamespaceEnsureContext {
            current: None,
            backend: crate::services::project_namespace_runtime::backend::build_backend(
                &controller.backend_factory,
                "/tmp/ccb-topo-test/.ccb/tmux.sock",
            )
            .unwrap(),
            session_is_alive: false,
            desired_socket_path: "/tmp/ccb-topo-test/.ccb/tmux.sock".to_string(),
            desired_session_name: "ccb-topo-test".to_string(),
            desired_layout_signature: None,
            desired_control_window_name: "control".to_string(),
            desired_workspace_window_name: "workspace".to_string(),
            topology_plan: None,
            recreate_cause: None,
        };
        let plan = TopologyPlan {
            signature: None,
            entry_window: "main".to_string(),
            windows: Vec::new(),
            sidebar_enabled: false,
        };
        let request = MaterializeTopologyRequest {
            controller: &controller,
            context: &context,
            topology_plan: &plan,
            epoch: 1,
            terminal_size: None,
            timeout_s: None,
        };
        let result = request.execute().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_topology_recreate_reason_workspace_changed() {
        let controller = test_controller();
        let state = crate::services::project_namespace_runtime::models::ProjectNamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 1,
            tmux_socket_path: "/tmp/ccb-topo-test/.ccb/tmux.sock".to_string(),
            tmux_session_name: "ccb-topo-test".to_string(),
            layout_version: 1,
            layout_signature: None,
            control_window_name: None,
            control_window_id: None,
            workspace_window_name: Some("old-workspace".to_string()),
            workspace_window_id: None,
            workspace_epoch: 1,
            ui_attachable: true,
            last_started_at: None,
            last_destroyed_at: None,
            last_destroy_reason: None,
        };
        let context = NamespaceEnsureContext {
            current: Some(state),
            backend: crate::services::project_namespace_runtime::backend::build_backend(
                &controller.backend_factory,
                "/tmp/ccb-topo-test/.ccb/tmux.sock",
            )
            .unwrap(),
            session_is_alive: true,
            desired_socket_path: "/tmp/ccb-topo-test/.ccb/tmux.sock".to_string(),
            desired_session_name: "ccb-topo-test".to_string(),
            desired_layout_signature: None,
            desired_control_window_name: "control".to_string(),
            desired_workspace_window_name: "new-workspace".to_string(),
            topology_plan: None,
            recreate_cause: None,
        };
        let plan = TopologyPlan {
            signature: None,
            entry_window: "new-workspace".to_string(),
            windows: vec![test_window("new-workspace", "cmd", &["claude"])],
            sidebar_enabled: false,
        };
        assert_eq!(
            topology_recreate_reason(&controller, &context, &plan),
            Some("topology_workspace_changed".to_string())
        );
    }

    #[test]
    fn test_existing_topology_agent_panes_empty_without_server() {
        let controller = test_controller();
        let context = NamespaceEnsureContext {
            current: None,
            backend: crate::services::project_namespace_runtime::backend::build_backend(
                &controller.backend_factory,
                "/tmp/ccb-topo-test/.ccb/tmux.sock",
            )
            .unwrap(),
            session_is_alive: false,
            desired_socket_path: "/tmp/ccb-topo-test/.ccb/tmux.sock".to_string(),
            desired_session_name: "ccb-topo-test".to_string(),
            desired_layout_signature: None,
            desired_control_window_name: "control".to_string(),
            desired_workspace_window_name: "workspace".to_string(),
            topology_plan: None,
            recreate_cause: None,
        };
        let plan = TopologyPlan {
            signature: None,
            entry_window: "main".to_string(),
            windows: vec![test_window("main", "claude", &["claude"])],
            sidebar_enabled: false,
        };
        let panes = existing_topology_agent_panes(&controller, &context, &plan);
        assert!(panes.is_empty());
    }

    #[test]
    #[ignore = "requires a running tmux server"]
    fn test_materialize_topology_noop_without_windows() {
        let controller = test_controller();
        let context = NamespaceEnsureContext {
            current: None,
            backend: crate::services::project_namespace_runtime::backend::build_backend(
                &controller.backend_factory,
                "/tmp/ccb-topo-test/.ccb/tmux.sock",
            )
            .unwrap(),
            session_is_alive: false,
            desired_socket_path: "/tmp/ccb-topo-test/.ccb/tmux.sock".to_string(),
            desired_session_name: "ccb-topo-test".to_string(),
            desired_layout_signature: None,
            desired_control_window_name: "control".to_string(),
            desired_workspace_window_name: "workspace".to_string(),
            topology_plan: None,
            recreate_cause: None,
        };
        let plan = TopologyPlan {
            signature: None,
            entry_window: "main".to_string(),
            windows: Vec::new(),
            sidebar_enabled: false,
        };
        let result = materialize_topology(&controller, &context, &plan, 1, None, None).unwrap();
        assert!(result.is_empty());
    }
}
