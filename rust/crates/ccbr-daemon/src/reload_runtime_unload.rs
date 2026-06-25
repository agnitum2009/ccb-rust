//! Mirrors Python `lib/ccbrd/reload_runtime_unload.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_plan::ReloadPlan;

/// Check for a blocker that requires pre-namespace unload before applying.
pub fn pre_namespace_unload_blocker(
    app: &CcbdApp,
    _old_graph: &ServiceGraph,
    plan: &ReloadPlan,
) -> Option<(String, String)> {
    let mut agents: Vec<String> = plan
        .operations
        .iter()
        .filter(|operation| operation.op == "remove_agent")
        .filter_map(|operation| {
            operation
                .details
                .get("agent")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .collect();
    agents.sort();
    agents.dedup();
    for agent_name in agents {
        if app.dispatcher.state.has_outstanding(&agent_name) {
            return Some((
                "agent_has_outstanding_work".to_string(),
                format!("cannot unload agent with outstanding work: {agent_name}"),
            ));
        }
        if app
            .registry
            .get(&agent_name)
            .is_some_and(|entry| matches!(entry.state.as_str(), "busy" | "running" | "active"))
        {
            return Some((
                "agent_busy".to_string(),
                format!("cannot unload busy agent: {agent_name}"),
            ));
        }
    }
    None
}
