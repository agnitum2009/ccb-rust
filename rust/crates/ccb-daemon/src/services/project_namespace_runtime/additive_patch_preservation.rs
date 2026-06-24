//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/additive_patch_preservation.py`.

use std::collections::HashMap;

use super::backend::Backend;
use super::topology_plan::NamespaceTopologyPlan;

/// Query backend for panes matching all expected user options.
///
/// Mirrors Python `_list_panes_by_user_options` from `materialize_topology.py`.
fn list_panes_by_user_options(backend: &Backend, expected: &HashMap<&str, &str>) -> Vec<String> {
    if expected.is_empty() {
        return Vec::new();
    }
    let options: Vec<&str> = expected.keys().copied().collect();
    let mut fmt = String::from("#{pane_id}");
    for opt in &options {
        fmt.push('\t');
        fmt.push_str(&format!("#{{{opt}}}"));
    }
    let output = match backend._tmux_run(&["list-panes", "-a", "-F", &fmt], false, true) {
        Ok(out) if out.success() => out.stdout,
        _ => return Vec::new(),
    };
    let mut matches = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').map(str::trim).collect();
        if parts.len() != options.len() + 1 {
            continue;
        }
        let pane_id = parts[0];
        if !pane_id.starts_with('%') {
            continue;
        }
        if options
            .iter()
            .enumerate()
            .all(|(i, opt)| parts[i + 1] == expected[*opt])
        {
            matches.push(pane_id.to_string());
        }
    }
    matches
}

/// Read a single pane user option from the backend.
///
/// Mirrors Python `_pane_option` from `materialize_topology.py`.
#[allow(dead_code)]
fn pane_option(backend: &Backend, pane_id: &str, option_name: &str) -> Option<String> {
    let fmt = format!("#{{{option_name}}}");
    let output = backend._tmux_run(&["display-message", "-p", "-t", pane_id, &fmt], false, true);
    match output {
        Ok(out) if out.success() => {
            let value = out.stdout.lines().next().unwrap_or("").trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        _ => None,
    }
}

/// Find existing agent panes that match the desired topology.
///
/// Mirrors Python `existing_topology_agent_panes`.
pub fn existing_topology_agent_panes(
    backend: &Backend,
    project_id: &str,
    topology_plan: &NamespaceTopologyPlan,
) -> HashMap<String, String> {
    let mut agent_panes = HashMap::new();
    for window in &topology_plan.windows {
        for agent_name in &window.agent_names {
            let mut expected: HashMap<&str, &str> = HashMap::new();
            expected.insert("@ccb_project_id", project_id);
            expected.insert("@ccb_role", "agent");
            expected.insert("@ccb_slot", agent_name);
            expected.insert("@ccb_window", &window.name);
            expected.insert("@ccb_managed_by", "ccbd");
            let matches = list_panes_by_user_options(backend, &expected);
            if matches.len() == 1 {
                agent_panes.insert(agent_name.clone(), matches[0].clone());
            }
        }
    }
    agent_panes
}

/// Snapshot the pane ids for agents that should be preserved.
///
/// Mirrors Python `snapshot_preserved_agent_panes`.
pub fn snapshot_preserved_agent_panes(
    backend: &Backend,
    project_id: &str,
    topology_plan: &NamespaceTopologyPlan,
    agents: &[&str],
) -> HashMap<String, String> {
    let expected: Vec<String> = agents.iter().map(|s| s.to_string()).collect();
    if expected.is_empty() {
        return HashMap::new();
    }
    let panes = existing_topology_agent_panes(backend, project_id, topology_plan);
    panes
        .into_iter()
        .filter(|(agent, _)| expected.contains(agent))
        .collect()
}

/// Assert that preserved agent pane ids did not change.
///
/// Mirrors Python `assert_preserved_agent_panes`.
pub fn assert_preserved_agent_panes(
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
    expected_agents: &[&str],
) -> Result<(), String> {
    let changed = preservation_changes(before, after, expected_agents);
    if changed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "preserved agent pane ids changed: {}",
            changed.join(" ")
        ))
    }
}

fn preservation_changes(
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
    expected_agents: &[&str],
) -> Vec<String> {
    let expected: std::collections::HashSet<String> =
        expected_agents.iter().map(|s| s.to_string()).collect();
    let before_keys: std::collections::HashSet<String> = before.keys().cloned().collect();
    let after_keys: std::collections::HashSet<String> = after.keys().cloned().collect();

    let mut missing_before: Vec<String> = expected.difference(&before_keys).cloned().collect();
    missing_before.sort();
    let mut missing_after: Vec<String> = if expected.is_empty() {
        before_keys.difference(&after_keys).cloned().collect()
    } else {
        expected.difference(&after_keys).cloned().collect()
    };
    missing_after.sort();
    let mut missing: Vec<String> = before_keys.difference(&after_keys).cloned().collect();
    missing.sort();
    let mut changed: Vec<String> = before
        .iter()
        .filter_map(|(agent, pane_id)| {
            after
                .get(agent)
                .filter(|after_id| *after_id != pane_id)
                .map(|_| agent.clone())
        })
        .collect();
    changed.sort();

    let mut details = Vec::new();
    details.extend(format_detail("missing_before", &missing_before));
    details.extend(format_detail("missing_after", &missing_after));
    details.extend(format_detail("missing", &missing));
    details.extend(format_detail("changed", &changed));
    details
}

fn format_detail(name: &str, values: &[String]) -> Vec<String> {
    if values.is_empty() {
        Vec::new()
    } else {
        vec![format!("{}={}", name, values.join(","))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::project_namespace_runtime::test_support::{FakeTmuxBackend, Pane};
    use crate::services::project_namespace_runtime::topology_plan::build_namespace_topology_plan;
    use ccb_agents::models::{ProjectConfig, WindowSpec};

    fn base_config() -> ProjectConfig {
        ProjectConfig {
            windows: Some(vec![WindowSpec {
                name: "main".to_string(),
                order: 0,
                layout_spec: "agent1, agent2".to_string(),
                agent_names: vec!["agent1".to_string(), "agent2".to_string()],
            }]),
            ..Default::default()
        }
    }

    fn topology(config: &ProjectConfig) -> NamespaceTopologyPlan {
        build_namespace_topology_plan(config, None, None)
    }

    fn seed_agent_pane(
        fake: &FakeTmuxBackend,
        pane_id: &str,
        project_id: &str,
        window: &str,
        agent: &str,
    ) {
        fake.with_state_mut(|state| {
            state.panes.insert(
                pane_id.to_string(),
                Pane {
                    id: pane_id.to_string(),
                    session: "ccb-test".to_string(),
                    window: window.to_string(),
                    ..Default::default()
                },
            );
            let mut options = HashMap::new();
            options.insert("@ccb_project_id".to_string(), project_id.to_string());
            options.insert("@ccb_role".to_string(), "agent".to_string());
            options.insert("@ccb_slot".to_string(), agent.to_string());
            options.insert("@ccb_window".to_string(), window.to_string());
            options.insert("@ccb_managed_by".to_string(), "ccbd".to_string());
            state.pane_options.insert(pane_id.to_string(), options);
        });
    }

    #[test]
    fn test_snapshot_preserved_agent_panes_filters_by_expected_agents() {
        let fake = FakeTmuxBackend::new();
        let backend = fake
            .backend_factory()
            .build("/tmp/ccb-test/.ccb/tmux.sock")
            .unwrap();
        backend
            ._tmux_run(
                &[
                    "new-session",
                    "-d",
                    "-s",
                    "ccb-test",
                    "-n",
                    "main",
                    "-c",
                    "/tmp",
                ],
                false,
                true,
            )
            .unwrap();
        seed_agent_pane(&fake, "%11", "proj-1", "main", "agent1");
        seed_agent_pane(&fake, "%12", "proj-1", "main", "agent2");
        fake.with_state_mut(|state| {
            if let Some(window) = state
                .sessions
                .get_mut("ccb-test")
                .and_then(|w| w.first_mut())
            {
                window.panes = vec!["%11".to_string(), "%12".to_string()];
            }
        });

        let plan = topology(&base_config());
        let snapshot = snapshot_preserved_agent_panes(
            &backend,
            "proj-1",
            &plan,
            &["agent1", "agent2", "agent-missing"],
        );

        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.get("agent1"), Some(&"%11".to_string()));
        assert_eq!(snapshot.get("agent2"), Some(&"%12".to_string()));
        assert!(!snapshot.contains_key("agent-missing"));
    }

    #[test]
    fn test_assert_preserved_agent_panes_passes_when_unchanged() {
        let mut before = HashMap::new();
        before.insert("agent1".to_string(), "%11".to_string());
        before.insert("agent2".to_string(), "%12".to_string());
        assert!(assert_preserved_agent_panes(&before, &before, &["agent1", "agent2"]).is_ok());
    }

    #[test]
    fn test_assert_preserved_agent_panes_fails_when_changed() {
        let mut before = HashMap::new();
        before.insert("agent1".to_string(), "%11".to_string());
        before.insert("agent2".to_string(), "%12".to_string());
        let mut after = HashMap::new();
        after.insert("agent1".to_string(), "%11".to_string());
        after.insert("agent2".to_string(), "%99".to_string());
        let err = assert_preserved_agent_panes(&before, &after, &["agent1", "agent2"]).unwrap_err();
        assert!(err.contains("changed=agent2"), "unexpected error: {err}");
    }
}
