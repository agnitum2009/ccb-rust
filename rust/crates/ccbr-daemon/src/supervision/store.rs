use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionRecord {
    pub agent_name: String,
    pub restart_count: u32,
    pub last_restart_at: Option<String>,
    pub last_failure_reason: Option<String>,
    pub backoff_seconds: u32,
    pub state: String,
    #[serde(default)]
    pub escalated: bool,
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

    pub fn record_restart(
        &mut self,
        agent_name: &str,
        reason: &str,
        base_backoff_seconds: u32,
        max_backoff_seconds: u32,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        let record = self
            .records
            .entry(agent_name.to_string())
            .or_insert_with(|| SupervisionRecord {
                agent_name: agent_name.into(),
                restart_count: 0,
                last_restart_at: None,
                last_failure_reason: None,
                backoff_seconds: 0,
                state: "starting".into(),
                escalated: false,
            });
        record.restart_count += 1;
        record.last_restart_at = Some(now);
        record.last_failure_reason = Some(reason.into());
        record.backoff_seconds = crate::supervision::backoff::compute_backoff(
            record.restart_count,
            base_backoff_seconds,
            max_backoff_seconds,
        );
        record.state = "recovering".into();
    }

    pub fn record_success(&mut self, agent_name: &str) {
        if let Some(record) = self.records.get_mut(agent_name) {
            record.restart_count = 0;
            record.backoff_seconds = 1;
            record.state = "idle".into();
            record.last_failure_reason = None;
            record.escalated = false;
        }
    }

    pub fn record_escalation(&mut self, agent_name: &str, reason: &str) {
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
                escalated: false,
            });
        record.escalated = true;
        record.state = "escalated".into();
        record.last_failure_reason = Some(reason.into());
    }

    pub fn can_restart(
        &self,
        agent_name: &str,
        max_retries: u32,
        now: &chrono::DateTime<chrono::Utc>,
    ) -> bool {
        let Some(record) = self.records.get(agent_name) else {
            return true;
        };
        if record.escalated || record.restart_count >= max_retries {
            return false;
        }
        !crate::supervision::backoff::is_in_backoff_window(
            record.last_restart_at.as_deref(),
            record.backoff_seconds,
            now,
        )
    }

    /// Legacy predicate used by callers that do not yet integrate backoff.
    pub fn should_restart(&self, agent_name: &str) -> bool {
        self.records
            .get(agent_name)
            .is_none_or(|r| !r.escalated && r.restart_count < 5)
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
