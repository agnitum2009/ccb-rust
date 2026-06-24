use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountRecord {
    pub agent_name: String,
    pub pane_id: String,
    pub provider: String,
    pub workspace_path: String,
    pub mounted_at: String,
    pub state: String,
}

impl MountRecord {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "agent_name": self.agent_name,
            "pane_id": self.pane_id,
            "provider": self.provider,
            "workspace_path": self.workspace_path,
            "mounted_at": self.mounted_at,
            "state": self.state,
        })
    }
}

pub struct MountService {
    mounts: std::collections::HashMap<String, MountRecord>,
}

impl MountService {
    pub fn new() -> Self {
        Self {
            mounts: std::collections::HashMap::new(),
        }
    }

    pub fn mount(&mut self, record: MountRecord) {
        self.mounts.insert(record.agent_name.clone(), record);
    }

    pub fn unmount(&mut self, agent_name: &str) -> Option<MountRecord> {
        self.mounts.remove(agent_name)
    }

    pub fn get(&self, agent_name: &str) -> Option<&MountRecord> {
        self.mounts.get(agent_name)
    }

    pub fn all_mounts(&self) -> Vec<&MountRecord> {
        self.mounts.values().collect()
    }
}

impl Default for MountService {
    fn default() -> Self {
        Self::new()
    }
}
