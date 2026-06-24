//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/patch_validation_targets.py`.
//! Compute which (window, agent) pairs were removed during a reload.

#![allow(dead_code)]

use std::collections::HashSet;

use crate::reload_additive_agents::TopologyWindow;

/// Return (window, agent) pairs that exist in old topology but not new.
pub(crate) fn removed_agent_targets(
    old_topology: &[TopologyWindow],
    new_topology: &[TopologyWindow],
) -> HashSet<(String, String)> {
    let new_pairs = topology_agent_pairs(new_topology);
    topology_agent_pairs(old_topology)
        .into_iter()
        .filter(|p| !new_pairs.contains(p))
        .collect()
}

fn topology_agent_pairs(topology: &[TopologyWindow]) -> HashSet<(String, String)> {
    let mut pairs = HashSet::new();
    for window in topology {
        for agent in &window.agent_names {
            pairs.insert((window.name.clone(), agent.clone()));
        }
    }
    pairs
}
