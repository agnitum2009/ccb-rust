use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleReport {
    pub project_id: String,
    pub event: String,
    pub timestamp: String,
    pub details: serde_json::Value,
}

pub struct LifecycleService {
    reports: Vec<LifecycleReport>,
}

impl LifecycleService {
    pub fn new() -> Self {
        Self {
            reports: Vec::new(),
        }
    }

    pub fn record(&mut self, report: LifecycleReport) {
        self.reports.push(report);
    }

    pub fn recent_reports(&self, limit: usize) -> &[LifecycleReport] {
        let start = self.reports.len().saturating_sub(limit);
        &self.reports[start..]
    }

    pub fn all_reports(&self) -> &[LifecycleReport] {
        &self.reports
    }
}

impl Default for LifecycleService {
    fn default() -> Self {
        Self::new()
    }
}
