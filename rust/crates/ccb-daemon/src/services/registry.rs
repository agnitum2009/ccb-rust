use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeEntry {
    pub agent_name: String,
    pub provider: String,
    pub state: String,
    pub health: String,
    #[serde(default)]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub runtime_pid: Option<u32>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub restart_count: u32,
}

impl AgentRuntimeEntry {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "agent_name": self.agent_name,
            "provider": self.provider,
            "state": self.state,
            "health": self.health,
            "pane_id": self.pane_id,
            "workspace_path": self.workspace_path,
            "runtime_pid": self.runtime_pid,
            "session_id": self.session_id,
            "restart_count": self.restart_count,
        })
    }
}

pub struct AgentRegistry {
    entries: std::collections::HashMap<String, AgentRuntimeEntry>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, entry: AgentRuntimeEntry) {
        self.entries.insert(entry.agent_name.clone(), entry);
    }

    pub fn get(&self, agent_name: &str) -> Option<&AgentRuntimeEntry> {
        self.entries.get(agent_name)
    }

    pub fn get_mut(&mut self, agent_name: &str) -> Option<&mut AgentRuntimeEntry> {
        self.entries.get_mut(agent_name)
    }

    pub fn remove(&mut self, agent_name: &str) -> Option<AgentRuntimeEntry> {
        self.entries.remove(agent_name)
    }

    pub fn all_entries(&self) -> Vec<&AgentRuntimeEntry> {
        self.entries.values().collect()
    }

    pub fn set_state(&mut self, agent_name: &str, state: &str, health: &str) {
        if let Some(entry) = self.entries.get_mut(agent_name) {
            entry.state = state.into();
            entry.health = health.into();
        }
    }

    pub fn update_pane_id(&mut self, agent_name: &str, pane_id: &str) {
        if let Some(entry) = self.entries.get_mut(agent_name) {
            entry.pane_id = Some(pane_id.to_string());
            if entry.state == "registered" {
                entry.state = "idle".into();
            }
            if entry.health == "unknown" {
                entry.health = "healthy".into();
            }
        } else {
            self.register(AgentRuntimeEntry {
                agent_name: agent_name.to_string(),
                provider: String::new(),
                state: "idle".into(),
                health: "healthy".into(),
                pane_id: Some(pane_id.to_string()),
                workspace_path: None,
                runtime_pid: None,
                session_id: None,
                restart_count: 0,
            });
        }
    }

    /// Mark an agent as stopped and clear runtime binding fields.
    /// Mirrors the Python stop flow registry update.
    pub fn mark_stopped(&mut self, agent_name: &str) {
        if let Some(entry) = self.entries.get_mut(agent_name) {
            entry.state = "stopped".into();
            entry.health = "stopped".into();
            entry.pane_id = None;
            entry.runtime_pid = None;
            entry.session_id = None;
            entry.restart_count = 0;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_stopped_clears_runtime_fields() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentRuntimeEntry {
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            state: "running".into(),
            health: "healthy".into(),
            pane_id: Some("%1".to_string()),
            workspace_path: None,
            runtime_pid: Some(1234),
            session_id: Some("sess-1".to_string()),
            restart_count: 2,
        });

        registry.mark_stopped("claude");

        let entry = registry.get("claude").unwrap();
        assert_eq!(entry.state, "stopped");
        assert_eq!(entry.health, "stopped");
        assert!(entry.pane_id.is_none());
        assert!(entry.runtime_pid.is_none());
        assert!(entry.session_id.is_none());
        assert_eq!(entry.restart_count, 0);
    }
}
