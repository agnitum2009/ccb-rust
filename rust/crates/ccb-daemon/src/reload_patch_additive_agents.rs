//! Mirrors Python `lib/ccbd/reload_patch_additive_agents.py`.

#![allow(dead_code)]

use crate::reload_additive_agents::{
    append_agent_plan_for_window, window_agent_names, TopologyWindow,
};
use crate::reload_plan::NamespacePatchStep;
use serde_json::json;
use std::collections::HashMap;

/// Factory for additive namespace patch steps.
type StepFactoryFn<'a> = &'a dyn Fn(
    &str,
    &str,
    &str,
    &str,
    &str,
    Option<&str>,
    &str,
) -> NamespacePatchStep;

/// A single blocked additive step with a human-readable reason.
#[derive(Debug, Clone)]
pub struct BlockedAdditiveStep {
    pub op: String,
    pub window: String,
    pub reason: String,
}

/// Compute additive agent steps for windows that only append new agents.
pub(crate) fn additive_agent_steps(
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
    step_factory: StepFactoryFn<'_>,
) -> AdditiveAgentStepsResult {
    let old_windows = window_map(old_topology);
    let new_windows = window_map(new_topology);
    let added_windows: std::collections::HashSet<String> = new_windows
        .keys()
        .filter(|w| !old_windows.contains_key(*w))
        .cloned()
        .collect();

    let mut steps: Vec<NamespacePatchStep> = Vec::new();
    let mut blocked: Vec<BlockedAdditiveStep> = Vec::new();

    for (window_name, new_window) in &new_windows {
        if added_windows.contains(window_name) {
            continue;
        }
        let old_window = match old_windows.get(window_name) {
            Some(w) => *w,
            None => continue,
        };
        let result = steps_for_window(
            window_name,
            old_window,
            new_window,
            step_factory,
        );
        steps.extend(result.steps);
        blocked.extend(result.blocked);
    }

    AdditiveAgentStepsResult { steps, blocked }
}

/// Result of computing additive agent steps.
#[derive(Debug, Clone)]
pub struct AdditiveAgentStepsResult {
    pub steps: Vec<NamespacePatchStep>,
    pub blocked: Vec<BlockedAdditiveStep>,
}

fn steps_for_window(
    window_name: &str,
    old_window: &TopologyWindow,
    new_window: &TopologyWindow,
    step_factory: StepFactoryFn<'_>,
) -> AdditiveAgentStepsResult {
    let old_agents = window_agent_names(old_window);
    let new_agents = window_agent_names(new_window);

    if old_agents == new_agents {
        return AdditiveAgentStepsResult {
            steps: Vec::new(),
            blocked: Vec::new(),
        };
    }
    if new_agents.len() < old_agents.len() {
        return AdditiveAgentStepsResult {
            steps: Vec::new(),
            blocked: Vec::new(),
        };
    }
    if new_agents[..old_agents.len()] != old_agents {
        return AdditiveAgentStepsResult {
            steps: Vec::new(),
            blocked: vec![blocked_append(
                window_name,
                "Phase 5 additive patch only supports appending new agents after existing panes",
            )],
        };
    }

    let append_plan = match append_agent_plan_for_window(old_window, new_window) {
        Some(plan) => plan,
        None => {
            return AdditiveAgentStepsResult {
                steps: Vec::new(),
                blocked: vec![blocked_append(
                    window_name,
                    "Phase 5 additive patch only supports expanding the last existing agent pane",
                )],
            }
        }
    };

    AdditiveAgentStepsResult {
        steps: append_steps(window_name, &old_agents, &append_plan, step_factory),
        blocked: Vec::new(),
    }
}

fn append_steps(
    window_name: &str,
    old_agents: &[String],
    append_plan: &[crate::reload_append_layout::AppendAgentPlan],
    step_factory: StepFactoryFn<'_>,
) -> Vec<NamespacePatchStep> {
    let mut anchor = old_agents.last().map(|s| s.as_str());
    let mut steps = Vec::new();
    for append in append_plan {
        steps.push(step_factory(
            "create_agent_pane",
            window_name,
            &append.agent,
            "agent",
            &append.agent,
            anchor,
            "new agent appended to existing managed window",
        ));
        anchor = Some(&append.agent);
    }
    steps
}

fn blocked_append(window_name: &str, reason: &str) -> BlockedAdditiveStep {
    BlockedAdditiveStep {
        op: "add_agent".to_string(),
        window: window_name.to_string(),
        reason: reason.to_string(),
    }
}

fn window_map(topology: &[TopologyWindow]) -> HashMap<String, &TopologyWindow> {
    topology.iter().map(|w| (w.name.clone(), w)).collect()
}

impl BlockedAdditiveStep {
    pub fn to_record(&self) -> serde_json::Value {
        json!({
            "op": self.op,
            "window": self.window,
            "reason": self.reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload_additive_agents::build_namespace_topology;
    use ccb_agents::models::ProjectConfig;

    fn step_factory(
        action: &str,
        window: &str,
        agent: &str,
        role: &str,
        slot_key: &str,
        anchor_agent: Option<&str>,
        reason: &str,
    ) -> NamespacePatchStep {
        NamespacePatchStep {
            action: action.to_string(),
            window: Some(window.to_string()),
            agent: Some(agent.to_string()),
            role: Some(role.to_string()),
            slot_key: Some(slot_key.to_string()),
            reason: Some(reason.to_string()),
            anchor_agent: anchor_agent.map(|s| s.to_string()),
        }
    }

    fn config_with_window(layout: &str, agents: &[&str]) -> ProjectConfig {
        let mut config = ProjectConfig::default();
        config.windows = Some(vec![ccb_agents::models::WindowSpec {
            name: "main".to_string(),
            order: 0,
            layout_spec: layout.to_string(),
            agent_names: agents.iter().map(|s| s.to_string()).collect(),
        }]);
        for agent in agents {
            config.agents.insert(
                agent.to_string(),
                ccb_agents::models::AgentSpec::default_with_name(agent),
            );
        }
        config
    }

    #[test]
    fn test_additive_agent_steps_no_change() {
        let config = config_with_window("claude", &["claude"]);
        let topology = build_namespace_topology(&config);
        let result = additive_agent_steps(&topology, &topology, &step_factory);
        assert!(result.steps.is_empty());
        assert!(result.blocked.is_empty());
    }

    #[test]
    fn test_additive_agent_steps_appends_one() {
        let old_config = config_with_window("claude", &["claude"]);
        let new_config = config_with_window("claude;codex", &["claude", "codex"]);
        let old_topology = build_namespace_topology(&old_config);
        let new_topology = build_namespace_topology(&new_config);
        let result = additive_agent_steps(&old_topology, &new_topology, &step_factory);
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].agent, Some("codex".to_string()));
        assert_eq!(result.steps[0].window, Some("main".to_string()));
        assert_eq!(result.steps[0].anchor_agent, Some("claude".to_string()));
        assert!(result.blocked.is_empty());
    }

    #[test]
    fn test_additive_agent_steps_blocked_when_reorder() {
        let old_config = config_with_window("claude;codex", &["claude", "codex"]);
        let new_config = config_with_window("codex;claude", &["codex", "claude"]);
        let old_topology = build_namespace_topology(&old_config);
        let new_topology = build_namespace_topology(&new_config);
        let result = additive_agent_steps(&old_topology, &new_topology, &step_factory);
        assert!(result.steps.is_empty());
        assert_eq!(result.blocked.len(), 1);
        assert_eq!(result.blocked[0].window, "main");
    }

    #[test]
    fn test_additive_agent_steps_skips_added_windows() {
        let mut old_config = ProjectConfig::default();
        old_config.windows = Some(vec![]);
        let new_config = config_with_window("claude", &["claude"]);
        let old_topology = build_namespace_topology(&old_config);
        let new_topology = build_namespace_topology(&new_config);
        let result = additive_agent_steps(&old_topology, &new_topology, &step_factory);
        assert!(result.steps.is_empty());
        assert!(result.blocked.is_empty());
    }
}
