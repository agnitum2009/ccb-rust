//! Mirrors Python `lib/ccbd/reload_additive_agents.py`.
//!
//! Additive namespace steps: create windows/panes for agents that exist only
//! in the new config during a reload.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use ccb_agents::models::ProjectConfig;

use crate::reload_append_layout::{
    rightmost_leaf_append_plan, AppendAgentPlan, WindowLayoutAccess,
};
use crate::reload_plan::{NamespacePatchStep, ReloadOperation};

#[derive(Debug, Clone)]
pub(crate) struct TopologyWindow {
    pub name: String,
    pub agent_names: Vec<String>,
    pub user_layout: String,
    pub is_tool: bool,
}

pub(crate) fn build_namespace_topology(config: &ProjectConfig) -> Vec<TopologyWindow> {
    let mut windows: Vec<TopologyWindow> = Vec::new();
    if let Some(ws) = &config.windows {
        for w in ws {
            windows.push(TopologyWindow {
                name: w.name.clone(),
                agent_names: w.agent_names.clone(),
                user_layout: w.layout_spec.clone(),
                is_tool: false,
            });
        }
    }
    if let Some(tools) = &config.tool_windows {
        for t in tools {
            windows.push(TopologyWindow {
                name: t.name.clone(),
                agent_names: Vec::new(),
                user_layout: String::new(),
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
            anchor_agent: None,
            reason: Some("window exists only in new config".into()),
        });
        for agent_name in &window.agent_names {
            steps.push(NamespacePatchStep {
                action: "create_agent_pane".into(),
                window: Some(window.name.clone()),
                agent: Some(agent_name.clone()),
                role: Some("agent".into()),
                slot_key: Some(agent_name.clone()),
                anchor_agent: None,
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
                anchor_agent: None,
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
                    anchor_agent: None,
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

impl WindowLayoutAccess for TopologyWindow {
    fn user_layout(&self) -> String {
        self.user_layout.clone()
    }
    fn agent_names(&self) -> Vec<String> {
        self.agent_names.clone()
    }
}

/// Return agent names for a topology window.
pub(crate) fn window_agent_names(window: &TopologyWindow) -> Vec<String> {
    window
        .agent_names
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build a map from window name to window for a topology.
pub(crate) fn window_map(topology: &[TopologyWindow]) -> HashMap<String, &TopologyWindow> {
    topology.iter().map(|w| (w.name.clone(), w)).collect()
}

/// Build append plans for all windows that grow additively.
pub(crate) fn append_agent_windows(
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> Option<HashMap<String, Vec<AppendAgentPlan>>> {
    let old_windows = window_map(old_topology);
    let new_windows = window_map(new_topology);
    let added_windows: HashSet<String> = new_windows
        .keys()
        .filter(|w| !old_windows.contains_key(*w))
        .cloned()
        .collect();
    let mut append = HashMap::new();
    for (window_name, new_window) in &new_windows {
        if added_windows.contains(window_name) {
            continue;
        }
        let old_window = match old_windows.get(window_name) {
            Some(w) => *w,
            None => continue,
        };
        let plan = append_agent_plan_for_window(old_window, new_window)?;
        if !plan.is_empty() {
            append.insert(window_name.clone(), plan);
        }
    }
    Some(append)
}

/// Build an append plan for a single window, or None if not appendable.
pub(crate) fn append_agent_plan_for_window(
    old_window: &TopologyWindow,
    new_window: &TopologyWindow,
) -> Option<Vec<AppendAgentPlan>> {
    let old_agents = window_agent_names(old_window);
    let new_agents = window_agent_names(new_window);
    if old_agents == new_agents {
        return Some(Vec::new());
    }
    if new_agents.len() < old_agents.len() {
        return Some(Vec::new());
    }
    if new_agents[..old_agents.len()] != old_agents {
        return None;
    }
    let append_plan = rightmost_leaf_append_plan(old_window, new_window)?;
    let appended_names: Vec<String> = append_plan.iter().map(|p| p.agent.clone()).collect();
    if appended_names != new_agents[old_agents.len()..] {
        return None;
    }
    Some(append_plan)
}

/// Return (window, agent) pairs for agents present in the new topology but not the old.
pub(crate) fn new_agent_targets(
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> HashSet<(String, String)> {
    let old_pairs = agent_window_pairs(old_topology);
    new_topology
        .iter()
        .flat_map(|w| {
            window_agent_names(w)
                .into_iter()
                .map(move |a| (w.name.clone(), a))
        })
        .filter(|pair| !old_pairs.contains(pair))
        .collect()
}

/// Return all (window, agent) pairs in a topology.
pub(crate) fn agent_window_pairs(topology: &[TopologyWindow]) -> HashSet<(String, String)> {
    topology
        .iter()
        .flat_map(|w| {
            window_agent_names(w)
                .into_iter()
                .map(move |a| (w.name.clone(), a))
        })
        .collect()
}
