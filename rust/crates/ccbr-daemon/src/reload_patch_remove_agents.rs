//! Mirrors Python `lib/ccbrd/reload_patch_remove_agents.py`.
//!
//! Remove namespace steps: kill panes/windows for agents/tool-windows removed
//! during a reload.

use std::collections::{HashMap, HashSet};

use crate::reload_additive_agents::TopologyWindow;
use crate::reload_plan::{NamespacePatchStep, ReloadOperation};

pub(crate) fn remove_agent_steps(
    operations: &[ReloadOperation],
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> Vec<NamespacePatchStep> {
    let _ = new_topology;
    let old_map: HashMap<String, &TopologyWindow> =
        old_topology.iter().map(|w| (w.name.clone(), w)).collect();

    let removed_agents: Vec<String> = operations
        .iter()
        .filter(|o| o.op == "remove_agent")
        .filter_map(|o| {
            o.details
                .get("agent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut steps: Vec<NamespacePatchStep> = Vec::new();
    for agent_name in removed_agents {
        let window_name = old_topology
            .iter()
            .find(|w| w.agent_names.contains(&agent_name))
            .map(|w| w.name.clone());
        if let Some(ref window) = window_name {
            if old_map.contains_key(window) {
                steps.push(NamespacePatchStep {
                    action: "kill_agent_pane".into(),
                    window: Some(window.clone()),
                    agent: Some(agent_name.clone()),
                    role: Some("agent".into()),
                    slot_key: Some(agent_name.clone()),
                    anchor_agent: None,
                    reason: Some("agent exists only in current published config".into()),
                });
            }
        }
    }
    steps
}

pub(crate) fn remove_tool_window_steps(
    operations: &[ReloadOperation],
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> Vec<NamespacePatchStep> {
    let new_map: HashMap<String, &TopologyWindow> =
        new_topology.iter().map(|w| (w.name.clone(), w)).collect();
    let old_map: HashMap<String, &TopologyWindow> =
        old_topology.iter().map(|w| (w.name.clone(), w)).collect();

    let removed_tools: Vec<String> = operations
        .iter()
        .filter(|o| o.op == "remove_tool_window")
        .filter_map(|o| {
            o.details
                .get("window")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut steps: Vec<NamespacePatchStep> = Vec::new();
    for window_name in removed_tools {
        if let Some(window) = old_map.get(&window_name) {
            if window.is_tool && !new_map.contains_key(&window_name) {
                steps.push(NamespacePatchStep {
                    action: "kill_tool_window".into(),
                    window: Some(window_name.clone()),
                    agent: None,
                    role: Some("tool".into()),
                    slot_key: Some(format!("tool:{}", window_name)),
                    anchor_agent: None,
                    reason: Some(
                        "managed tool window exists only in current published config".into(),
                    ),
                });
            }
        }
    }
    steps
}

pub(crate) fn missing_remove_agent_steps(
    operations: &[ReloadOperation],
    steps: &[NamespacePatchStep],
) -> Vec<serde_json::Value> {
    let expected: HashSet<String> = operations
        .iter()
        .filter(|o| o.op == "remove_agent")
        .filter_map(|o| {
            o.details
                .get("agent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    let planned: HashSet<String> = steps
        .iter()
        .filter(|s| s.action == "kill_agent_pane" && s.agent.is_some())
        .map(|s| s.agent.clone().unwrap())
        .collect();
    expected
        .difference(&planned)
        .map(|agent_name| {
            serde_json::json!({
                "op": "remove_agent",
                "agent": agent_name,
                "reason": "remove_agent operation was not covered by a namespace pane removal step",
            })
        })
        .collect()
}

pub(crate) fn missing_tool_window_steps(
    operations: &[ReloadOperation],
    steps: &[NamespacePatchStep],
) -> Vec<serde_json::Value> {
    let created: HashSet<String> = steps
        .iter()
        .filter(|s| s.action == "create_tool_pane" && s.window.is_some())
        .map(|s| s.window.clone().unwrap())
        .collect();
    let removed: HashSet<String> = steps
        .iter()
        .filter(|s| s.action == "kill_tool_window" && s.window.is_some())
        .map(|s| s.window.clone().unwrap())
        .collect();

    let mut missing: Vec<serde_json::Value> = Vec::new();
    for op in operations {
        if op.op == "add_tool_window" {
            if let Some(window) = op.details.get("window").and_then(|v| v.as_str()) {
                if !created.contains(window) {
                    missing.push(serde_json::json!({
                        "op": "add_tool_window",
                        "window": window,
                        "reason": "add_tool_window operation was not covered by a tool pane creation step",
                    }));
                }
            }
        }
        if op.op == "remove_tool_window" {
            if let Some(window) = op.details.get("window").and_then(|v| v.as_str()) {
                if !removed.contains(window) {
                    missing.push(serde_json::json!({
                        "op": "remove_tool_window",
                        "window": window,
                        "reason": "remove_tool_window operation was not covered by a tool window removal step",
                    }));
                }
            }
        }
    }
    missing
}
