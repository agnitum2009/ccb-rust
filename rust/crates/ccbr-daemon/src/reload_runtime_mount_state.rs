//! Mirrors Python `lib/ccbd/reload_runtime_mount_state.py`.

use crate::reload_runtime_mount_validation::AgentRecord;
use std::collections::HashMap;

/// Extract a map of agent names to pane ids from a record.
pub fn agent_panes_from_record(
    value: &serde_json::Map<String, serde_json::Value>,
) -> HashMap<String, String> {
    let mut panes = HashMap::new();
    for (agent, pane) in value {
        let agent_name = agent.trim();
        let pane_id = pane.as_str().unwrap_or("").trim();
        if !agent_name.is_empty() && !pane_id.is_empty() {
            panes.insert(agent_name.to_string(), pane_id.to_string());
        }
    }
    panes
}

/// Normalize a collection of values into unique, non-empty agent names.
pub fn agent_names(value: &serde_json::Value) -> Vec<String> {
    let values = if let serde_json::Value::Object(obj) = value {
        obj.keys().cloned().collect::<Vec<_>>()
    } else if let serde_json::Value::Array(arr) = value {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    };
    let mut names = Vec::new();
    for item in values {
        let name = item.trim();
        if !name.is_empty() && !names.contains(&name.to_string()) {
            names.push(name.to_string());
        }
    }
    names
}

/// Build runtime snapshots for the given agent names.
pub fn runtime_snapshots(
    registry: &dyn RuntimeRegistry,
    agents: &[String],
) -> HashMap<String, Option<serde_json::Value>> {
    agents
        .iter()
        .map(|agent| {
            let record = registry.get(agent).as_ref().map(runtime_record);
            (agent.clone(), record)
        })
        .collect()
}

/// Convert an AgentRecord into a JSON record.
pub fn runtime_record(record: &AgentRecord) -> serde_json::Value {
    serde_json::json!({
        "state": record.state,
        "health": record.health,
        "desired_state": record.desired_state,
        "reconcile_state": record.reconcile_state,
    })
}

/// Trait abstracting runtime registry lookups.
pub trait RuntimeRegistry {
    fn get(&self, agent_name: &str) -> Option<AgentRecord>;
    fn list_all(&self) -> Vec<AgentRecord>;
    fn list_names(&self) -> Vec<String>;
}

/// Build the list of agents to guard during an additive mount.
pub fn runtime_guard_agents(
    registry: &dyn RuntimeRegistry,
    requested_agents: &[String],
    preserved_agents: &[String],
) -> Vec<String> {
    let requested: std::collections::HashSet<String> = requested_agents.iter().cloned().collect();
    let mut guarded = preserved_agents.to_vec();
    for name in registry.list_names() {
        let name = name.trim().to_string();
        if !name.is_empty() && !requested.contains(&name) && !guarded.contains(&name) {
            guarded.push(name);
        }
    }
    guarded
}

/// Return agents whose runtime record changed between before and after.
pub fn changed_agents(
    before: &HashMap<String, Option<serde_json::Value>>,
    after: &HashMap<String, Option<serde_json::Value>>,
) -> Vec<String> {
    let keys: std::collections::HashSet<String> =
        before.keys().chain(after.keys()).cloned().collect();
    let mut changed: Vec<String> = keys
        .into_iter()
        .filter(|agent| before.get(agent) != after.get(agent))
        .collect();
    changed.sort();
    changed
}

/// Extract started agent names from a summary object.
pub fn summary_started(summary: Option<&serde_json::Value>, fallback: &[String]) -> Vec<String> {
    let summary = match summary {
        Some(s) => s,
        None => return fallback.to_vec(),
    };
    let started = summary
        .get("started")
        .cloned()
        .or_else(|| summary.get("agent_results").cloned());
    match started {
        Some(serde_json::Value::Array(arr)) => {
            let names: Vec<String> = arr
                .iter()
                .filter_map(|v| {
                    if let Some(name) = v.as_str() {
                        Some(name.to_string())
                    } else {
                        v.get("agent_name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    }
                })
                .collect();
            if names.is_empty() {
                fallback.to_vec()
            } else {
                names
            }
        }
        Some(serde_json::Value::Object(obj)) => {
            let keys: Vec<String> = obj.keys().cloned().collect();
            if keys.is_empty() {
                fallback.to_vec()
            } else {
                keys
            }
        }
        _ => fallback.to_vec(),
    }
}

/// Convert a summary object into a JSON record.
pub fn summary_record(summary: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    summary.cloned()
}

/// Clean text value.
pub fn clean_text(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Parse an optional integer.
pub fn optional_int(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|v| v.as_i64())
}

/// Validate pane id format.
pub fn valid_pane_id(value: Option<&str>) -> bool {
    value.unwrap_or("").trim().starts_with('%')
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestRegistry {
        entries: HashMap<String, AgentRecord>,
    }

    impl RuntimeRegistry for TestRegistry {
        fn get(&self, agent_name: &str) -> Option<AgentRecord> {
            self.entries.get(agent_name).cloned()
        }
        fn list_all(&self) -> Vec<AgentRecord> {
            self.entries.values().cloned().collect()
        }
        fn list_names(&self) -> Vec<String> {
            self.entries.keys().cloned().collect()
        }
    }

    fn test_registry() -> TestRegistry {
        let mut entries = HashMap::new();
        entries.insert(
            "claude".to_string(),
            AgentRecord {
                state: Some("idle".to_string()),
                health: Some("healthy".to_string()),
                desired_state: None,
                reconcile_state: None,
                fields: HashMap::new(),
            },
        );
        entries.insert(
            "codex".to_string(),
            AgentRecord {
                state: Some("idle".to_string()),
                health: Some("healthy".to_string()),
                desired_state: None,
                reconcile_state: None,
                fields: HashMap::new(),
            },
        );
        TestRegistry { entries }
    }

    #[test]
    fn test_agent_panes_from_record() {
        let mut record = serde_json::Map::new();
        record.insert("claude".to_string(), serde_json::json!("%1"));
        record.insert("".to_string(), serde_json::json!("%2"));
        record.insert("codex".to_string(), serde_json::json!(""));
        let panes = agent_panes_from_record(&record);
        assert_eq!(panes.len(), 1);
        assert_eq!(panes.get("claude"), Some(&"%1".to_string()));
    }

    #[test]
    fn test_agent_names_from_object() {
        let value = serde_json::json!({"claude": true, "codex": true, "": true});
        let names = agent_names(&value);
        assert_eq!(names, vec!["claude", "codex"]);
    }

    #[test]
    fn test_agent_names_from_array() {
        let value = serde_json::json!(["claude", "", "codex", "claude"]);
        let names = agent_names(&value);
        assert_eq!(names, vec!["claude", "codex"]);
    }

    #[test]
    fn test_runtime_snapshots() {
        let registry = test_registry();
        let snapshots =
            runtime_snapshots(&registry, &["claude".to_string(), "missing".to_string()]);
        assert!(snapshots.contains_key("claude"));
        assert!(snapshots.get("claude").unwrap().is_some());
        assert!(snapshots.get("missing").unwrap().is_none());
    }

    #[test]
    fn test_runtime_guard_agents() {
        let registry = test_registry();
        let guarded =
            runtime_guard_agents(&registry, &["claude".to_string()], &["gemini".to_string()]);
        assert!(guarded.contains(&"gemini".to_string()));
        assert!(guarded.contains(&"codex".to_string()));
        assert!(!guarded.contains(&"claude".to_string()));
    }

    #[test]
    fn test_changed_agents() {
        let mut before = HashMap::new();
        before.insert("claude".to_string(), Some(serde_json::json!({"a": 1})));
        before.insert("codex".to_string(), Some(serde_json::json!({"a": 2})));
        let mut after = HashMap::new();
        after.insert("claude".to_string(), Some(serde_json::json!({"a": 1})));
        after.insert("codex".to_string(), Some(serde_json::json!({"a": 3})));
        after.insert("new".to_string(), Some(serde_json::json!({"a": 0})));
        let changed = changed_agents(&before, &after);
        assert_eq!(changed, vec!["codex", "new"]);
    }

    #[test]
    fn test_summary_started() {
        let summary = serde_json::json!({"started": ["claude", "codex"]});
        assert_eq!(
            summary_started(Some(&summary), &["fallback".to_string()]),
            vec!["claude", "codex"]
        );
        assert_eq!(
            summary_started(None, &["fallback".to_string()]),
            vec!["fallback"]
        );
    }

    #[test]
    fn test_summary_started_from_agent_results() {
        let summary = serde_json::json!({
            "agent_results": [
                {"agent_name": "claude", "status": "started"},
                {"agent_name": "codex", "status": "started"},
            ]
        });
        assert_eq!(
            summary_started(Some(&summary), &[]),
            vec!["claude", "codex"]
        );
    }

    #[test]
    fn test_valid_pane_id() {
        assert!(valid_pane_id(Some("%1")));
        assert!(!valid_pane_id(Some("1")));
        assert!(!valid_pane_id(None));
    }
}
