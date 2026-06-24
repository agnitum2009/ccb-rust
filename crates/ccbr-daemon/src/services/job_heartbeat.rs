use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHeartbeatRecord {
    pub job_id: String,
    pub agent_name: String,
    pub last_heartbeat_at: String,
    pub heartbeat_count: u32,
    pub status: String,
}

pub struct JobHeartbeatService {
    records: std::collections::HashMap<String, JobHeartbeatRecord>,
}

impl JobHeartbeatService {
    pub fn new() -> Self {
        Self {
            records: std::collections::HashMap::new(),
        }
    }

    pub fn record_heartbeat(&mut self, job_id: &str, agent_name: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let record = self
            .records
            .entry(job_id.to_string())
            .or_insert_with(|| JobHeartbeatRecord {
                job_id: job_id.into(),
                agent_name: agent_name.into(),
                last_heartbeat_at: now.clone(),
                heartbeat_count: 0,
                status: "active".into(),
            });
        record.last_heartbeat_at = now;
        record.heartbeat_count += 1;
    }

    pub fn get(&self, job_id: &str) -> Option<&JobHeartbeatRecord> {
        self.records.get(job_id)
    }

    pub fn remove(&mut self, job_id: &str) {
        self.records.remove(job_id);
    }

    pub fn stale_jobs(&self, threshold_secs: u64) -> Vec<&JobHeartbeatRecord> {
        let now = chrono::Utc::now();
        self.records
            .values()
            .filter(|r| {
                chrono::DateTime::parse_from_rfc3339(&r.last_heartbeat_at)
                    .ok()
                    .map(|t| {
                        (now - t.with_timezone(&chrono::Utc)).num_seconds() as u64 > threshold_secs
                    })
                    .unwrap_or(false)
            })
            .collect()
    }
}

impl Default for JobHeartbeatService {
    fn default() -> Self {
        Self::new()
    }
}
