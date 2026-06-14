//! Mirrors Python `lib/ccbd/reload_additive_agents.py`.
//!
//! Additive namespace steps: create windows/panes for agents that exist only
//! in the new config during a reload.

use std::collections::{HashMap, HashSet};

use ccb_agents::models::ProjectConfig;

use crate::reload_plan::{NamespacePatchStep, ReloadOperation};

#[derive(Debug, Clone)]
pub(crate) struct TopologyWindow {
    pub name: String,
    pub agent_names: Vec<String>,
    pub is_tool: bool,
}

pub(crate) fn build_namespace_topology(config: &ProjectConfig) -> Vec<TopologyWindow> {
    let mut windows: Vec<TopologyWindow> = Vec::new();
    if let Some(ws) = &config.windows {
        for w in ws {
            windows.push(TopologyWindow {
                name: w.name.clone(),
                agent_names: w.agent_names.clone(),
                is_tool: false,
            });
        }
    }
    if let Some(tools) = &config.tool_windows {
        for t in tools {
            windows.push(TopologyWindow {
                name: t.name.clone(),
                agent_names: Vec::new(),
                is_tool: true,
            });
        }
    }
    windows
}

pub(crate) fn additive_window_steps(
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> Vec<NamespacePatchStep> {
    let old_map: HashMap<String, &TopologyWindow> =
        old_topology.iter().map(|w| (w.name.clone(), w)).collect();
    let mut steps: Vec<NamespacePatchStep> = Vec::new();
    for window in new_topology {
        if old_map.contains_key(&window.name) {
            continue;
        }
        steps.push(NamespacePatchStep {
            action: "create_window".into(),
            window: Some(window.name.clone()),
            agent: None,
            role: None,
            slot_key: None,
            reason: Some("window exists only in new config".into()),
        });
        for agent_name in &window.agent_names {
            steps.push(NamespacePatchStep {
                action: "create_agent_pane".into(),
                window: Some(window.name.clone()),
                agent: Some(agent_name.clone()),
                role: Some("agent".into()),
                slot_key: Some(agent_name.clone()),
                reason: Some("new managed window needs an agent pane".into()),
            });
        }
        if window.is_tool {
            steps.push(NamespacePatchStep {
                action: "create_tool_pane".into(),
                window: Some(window.name.clone()),
                agent: None,
                role: Some("tool".into()),
                slot_key: Some(format!("tool:{}", window.name)),
                reason: Some("new managed tool window needs a tool pane".into()),
            });
        }
    }
    steps
}

pub(crate) fn additive_agent_steps(
    operations: &[ReloadOperation],
    _old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> Vec<NamespacePatchStep> {
    let new_map: HashMap<String, &TopologyWindow> =
        new_topology.iter().map(|w| (w.name.clone(), w)).collect();

    let added_agents: Vec<String> = operations
        .iter()
        .filter(|o| o.op == "add_agent")
        .filter_map(|o| {
            o.details
                .get("agent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut steps: Vec<NamespacePatchStep> = Vec::new();
    for agent_name in added_agents {
        let window_name = new_topology
            .iter()
            .find(|w| w.agent_names.contains(&agent_name))
            .map(|w| w.name.clone());
        if let Some(ref window) = window_name {
            if new_map.contains_key(window) {
                steps.push(NamespacePatchStep {
                    action: "create_agent_pane".into(),
                    window: Some(window.clone()),
                    agent: Some(agent_name.clone()),
                    role: Some("agent".into()),
                    slot_key: Some(agent_name.clone()),
                    reason: Some("agent exists only in new config".into()),
                });
            }
        }
    }
    steps
}

pub(crate) fn missing_additive_agent_steps(
    operations: &[ReloadOperation],
    steps: &[NamespacePatchStep],
) -> Vec<serde_json::Value> {
    let expected: HashSet<String> = operations
        .iter()
        .filter(|o| o.op == "add_agent")
        .filter_map(|o| {
            o.details
                .get("agent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    let planned: HashSet<String> = steps
        .iter()
        .filter(|s| s.action == "create_agent_pane" && s.agent.is_some())
        .map(|s| s.agent.clone().unwrap())
        .collect();
    expected
        .difference(&planned)
        .map(|agent_name| {
            serde_json::json!({
                "op": "add_agent",
                "agent": agent_name,
                "reason": "add_agent operation was not covered by an additive namespace patch step",
            })
        })
        .collect()
}
