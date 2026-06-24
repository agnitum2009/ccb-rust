use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionRecord {
    pub agent_name: String,
    pub restart_count: u32,
    pub last_restart_at: Option<String>,
    pub last_failure_reason: Option<String>,
    pub backoff_seconds: u32,
    pub state: String,
}

pub struct SupervisionStore {
    records: std::collections::HashMap<String, SupervisionRecord>,
}

impl SupervisionStore {
    pub fn new() -> Self {
        Self {
            records: std::collections::HashMap::new(),
        }
    }

    pub fn record_restart(&mut self, agent_name: &str, reason: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let record = self
            .records
            .entry(agent_name.to_string())
            .or_insert_with(|| SupervisionRecord {
                agent_name: agent_name.into(),
                restart_count: 0,
                last_restart_at: None,
                last_failure_reason: None,
                backoff_seconds: 1,
                state: "starting".into(),
            });
        record.restart_count += 1;
        record.last_restart_at = Some(now);
        record.last_failure_reason = Some(reason.into());
        record.backoff_seconds = std::cmp::min(record.backoff_seconds * 2, 300);
    }

    pub fn record_success(&mut self, agent_name: &str) {
        if let Some(record) = self.records.get_mut(agent_name) {
            record.backoff_seconds = 1;
            record.state = "idle".into();
        }
    }

    pub fn should_restart(&self, agent_name: &str) -> bool {
        self.records
            .get(agent_name)
            .is_none_or(|r| r.restart_count < 5)
    }

    pub fn get(&self, agent_name: &str) -> Option<&SupervisionRecord> {
        self.records.get(agent_name)
    }

    pub fn all_records(&self) -> Vec<&SupervisionRecord> {
        self.records.values().collect()
    }
}

impl Default for SupervisionStore {
    fn default() -> Self {
        Self::new()
    }
}
