use ccb_storage::jsonl::JsonlStore;
use ccb_storage::paths::PathLayout;

use crate::models::{JobEvent, JobRecord, SubmissionRecord, TargetKind};

/// Store for job records, persisted per target.
#[derive(Clone)]
pub struct JobStore {
    layout: PathLayout,
    jsonl: JsonlStore,
}

impl JobStore {
    pub fn new(layout: &PathLayout) -> Self {
        Self {
            layout: layout.clone(),
            jsonl: JsonlStore::new(),
        }
    }

    pub fn append(&self, record: &JobRecord) -> crate::Result<()> {
        let path = self.layout.target_jobs_path(
            &format!("{:?}", record.target_kind).to_lowercase(),
            &record.target_name,
        )?;
        self.jsonl.append(&path, record).map_err(Into::into)
    }

    pub fn list_agent(&self, agent_name: &str) -> Vec<JobRecord> {
        self.list_target(TargetKind::Agent, agent_name)
    }

    pub fn list_target(&self, target_kind: TargetKind, target_name: &str) -> Vec<JobRecord> {
        let Ok(path) = self
            .layout
            .target_jobs_path(&format!("{:?}", target_kind).to_lowercase(), target_name)
        else {
            return Vec::new();
        };
        self.jsonl.read_all(&path).unwrap_or_default()
    }

    pub fn list_agent_tail(&self, agent_name: &str, limit: usize) -> Vec<JobRecord> {
        self.list_target_tail(TargetKind::Agent, agent_name, limit)
    }

    pub fn list_target_tail(
        &self,
        target_kind: TargetKind,
        target_name: &str,
        limit: usize,
    ) -> Vec<JobRecord> {
        let Ok(path) = self
            .layout
            .target_jobs_path(&format!("{:?}", target_kind).to_lowercase(), target_name)
        else {
            return Vec::new();
        };
        self.jsonl.read_tail(&path, limit).unwrap_or_default()
    }

    pub fn get_latest(&self, agent_name: &str, job_id: &str) -> Option<JobRecord> {
        self.get_latest_target(TargetKind::Agent, agent_name, job_id)
    }

    pub fn get_latest_target(
        &self,
        target_kind: TargetKind,
        target_name: &str,
        job_id: &str,
    ) -> Option<JobRecord> {
        let Ok(path) = self
            .layout
            .target_jobs_path(&format!("{:?}", target_kind).to_lowercase(), target_name)
        else {
            return None;
        };
        self.jsonl
            .find_last(&path, |payload: &JobRecord| payload.job_id == job_id)
            .unwrap_or_default()
    }
}

/// Store for job events.
#[derive(Clone)]
pub struct JobEventStore {
    layout: PathLayout,
    jsonl: JsonlStore,
}

impl JobEventStore {
    pub fn new(layout: &PathLayout) -> Self {
        Self {
            layout: layout.clone(),
            jsonl: JsonlStore::new(),
        }
    }

    pub fn append(&self, event: &JobEvent) -> crate::Result<()> {
        let path = self.layout.target_events_path(
            &format!("{:?}", event.target_kind).to_lowercase(),
            &event.target_name,
        )?;
        self.jsonl.append(&path, event).map_err(Into::into)
    }

    pub fn read_since(&self, agent_name: &str, start_line: usize) -> (usize, Vec<JobEvent>) {
        self.read_since_target(TargetKind::Agent, agent_name, start_line)
    }

    pub fn read_since_target(
        &self,
        target_kind: TargetKind,
        target_name: &str,
        start_line: usize,
    ) -> (usize, Vec<JobEvent>) {
        let Ok(path) = self
            .layout
            .target_events_path(&format!("{:?}", target_kind).to_lowercase(), target_name)
        else {
            return (0, Vec::new());
        };
        let Ok((line_no, rows)) = self.jsonl.read_since::<JobEvent>(&path, start_line) else {
            return (0, Vec::new());
        };
        let events = rows
            .into_iter()
            .filter(|row| {
                row.event_type
                    .as_str()
                    .strip_prefix("job_")
                    .is_none_or(|_| true)
            })
            .collect();
        (line_no, events)
    }
}

/// Store for submission records.
#[derive(Clone)]
pub struct SubmissionStore {
    layout: PathLayout,
    jsonl: JsonlStore,
}

impl SubmissionStore {
    pub fn new(layout: &PathLayout) -> Self {
        Self {
            layout: layout.clone(),
            jsonl: JsonlStore::new(),
        }
    }

    pub fn append(&self, record: &SubmissionRecord) -> crate::Result<()> {
        let path = self.layout.ccbd_submissions_path();
        self.jsonl.append(&path, record).map_err(Into::into)
    }

    pub fn list_all(&self) -> Vec<SubmissionRecord> {
        let path = self.layout.ccbd_submissions_path();
        self.jsonl.read_all(&path).unwrap_or_default()
    }

    pub fn get_latest(&self, submission_id: &str) -> Option<SubmissionRecord> {
        let path = self.layout.ccbd_submissions_path();
        self.jsonl
            .find_last(&path, |payload: &SubmissionRecord| {
                payload.submission_id == submission_id
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DeliveryScope, JobStatus, MessageEnvelope, TargetKind};
    use tempfile::TempDir;

    fn make_job(job_id: &str, agent_name: &str) -> JobRecord {
        JobRecord {
            job_id: job_id.into(),
            submission_id: None,
            agent_name: agent_name.into(),
            provider: "claude".into(),
            request: MessageEnvelope {
                project_id: "p1".into(),
                to_agent: agent_name.into(),
                from_actor: "user".into(),
                body: "hello".into(),
                task_id: None,
                reply_to: None,
                message_type: "task_request".into(),
                delivery_scope: DeliveryScope::Agent,
                silence_on_success: false,
                route_options: serde_json::Value::Object(Default::default()),
                body_artifact: None,
            },
            status: JobStatus::Accepted,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
            workspace_path: None,
            target_kind: TargetKind::Agent,
            target_name: agent_name.into(),
            provider_instance: None,
            provider_options: serde_json::Value::Object(Default::default()),
        }
    }

    #[test]
    fn test_job_store_round_trip() {
        let dir = TempDir::new().unwrap();
        let p = camino::Utf8Path::from_path(dir.path()).unwrap();
        let layout = PathLayout::new(p);
        let store = JobStore::new(&layout);
        store.append(&make_job("job1", "claude")).unwrap();
        let jobs = store.list_agent("claude");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "job1");
    }
}
