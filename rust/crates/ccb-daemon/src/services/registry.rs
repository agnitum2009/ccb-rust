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
