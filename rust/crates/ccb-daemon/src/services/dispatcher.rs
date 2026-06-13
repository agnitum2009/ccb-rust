use std::collections::HashMap;

use crate::models::api_models::common::JobStatus;
use crate::models::api_models::records::JobRecord;

#[derive(Debug, Default)]
pub struct DispatcherState {
    queues: HashMap<String, Vec<String>>,
    job_index: HashMap<String, String>,
    active_jobs: HashMap<String, String>,
}

impl DispatcherState {
    pub fn new(agent_names: &[String]) -> Self {
        let mut queues = HashMap::new();
        for name in agent_names {
            queues.insert(name.clone(), Vec::new());
        }
        Self {
            queues,
            ..Default::default()
        }
    }

    pub fn rebuild(&mut self, jobs: &[JobRecord]) {
        self.job_index.clear();
        self.active_jobs.clear();
        for queue in self.queues.values_mut() {
            queue.clear();
        }
        for job in jobs {
            self.job_index
                .insert(job.job_id.clone(), job.agent_name.clone());
            if job.status == JobStatus::Running {
                self.active_jobs
                    .insert(job.agent_name.clone(), job.job_id.clone());
            } else if !job.status.is_terminal() {
                self.queues
                    .entry(job.agent_name.clone())
                    .or_default()
                    .push(job.job_id.clone());
            }
        }
    }

    pub fn active_job(&self, agent_name: &str) -> Option<&str> {
        self.active_jobs.get(agent_name).map(|s| s.as_str())
    }

    pub fn has_outstanding(&self, agent_name: &str) -> bool {
        self.active_jobs.contains_key(agent_name)
            || self.queues.get(agent_name).is_some_and(|q| !q.is_empty())
    }

    pub fn queue_depth(&self, agent_name: &str) -> usize {
        self.queues.get(agent_name).map_or(0, |q| q.len())
    }
}

pub struct JobDispatcher {
    pub state: DispatcherState,
    pub job_store: Vec<JobRecord>,
    pub agent_names: Vec<String>,
}

impl JobDispatcher {
    pub fn new(agent_names: Vec<String>) -> Self {
        let state = DispatcherState::new(&agent_names);
        Self {
            state,
            job_store: Vec::new(),
            agent_names,
        }
    }

    pub fn submit(
        &mut self,
        envelope: &crate::models::api_models::messages::MessageEnvelope,
    ) -> crate::models::api_models::receipts::SubmitReceipt {
        let now = chrono::Utc::now().to_rfc3339();
        let job_id = format!(
            "job_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        let job = JobRecord {
            job_id: job_id.clone(),
            submission_id: None,
            agent_name: envelope.to_agent.clone(),
            provider: String::new(),
            request: envelope.clone(),
            status: JobStatus::Accepted,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            workspace_path: None,
            target_kind: crate::models::api_models::common::TargetKind::Agent,
            target_name: envelope.to_agent.clone(),
        };
        self.job_store.push(job.clone());
        self.state.rebuild(&self.job_store);

        crate::models::api_models::receipts::SubmitReceipt {
            accepted_at: now,
            jobs: vec![crate::models::api_models::receipts::AcceptedJobReceipt {
                job_id,
                agent_name: envelope.to_agent.clone(),
                status: JobStatus::Accepted,
                accepted_at: chrono::Utc::now().to_rfc3339(),
                target_kind: crate::models::api_models::common::TargetKind::Agent,
                target_name: envelope.to_agent.clone(),
                provider_instance: None,
            }],
            submission_id: None,
        }
    }

    pub fn cancel(&mut self, job_id: &str) -> crate::models::api_models::receipts::CancelReceipt {
        let now = chrono::Utc::now().to_rfc3339();
        for job in &mut self.job_store {
            if job.job_id == job_id {
                job.status = JobStatus::Cancelled;
                job.cancel_requested_at = Some(now.clone());
                job.updated_at = now.clone();
                return crate::models::api_models::receipts::CancelReceipt {
                    job_id: job_id.to_string(),
                    agent_name: job.agent_name.clone(),
                    status: JobStatus::Cancelled,
                    cancelled_at: now,
                    target_kind: crate::models::api_models::common::TargetKind::Agent,
                    target_name: job.agent_name.clone(),
                };
            }
        }
        crate::models::api_models::receipts::CancelReceipt {
            job_id: job_id.to_string(),
            agent_name: String::new(),
            status: JobStatus::Cancelled,
            cancelled_at: now,
            target_kind: crate::models::api_models::common::TargetKind::Agent,
            target_name: String::new(),
        }
    }

    pub fn get(&self, job_id: &str) -> Option<&JobRecord> {
        self.job_store.iter().find(|j| j.job_id == job_id)
    }

    pub fn latest_for_agent(&self, agent_name: &str) -> Option<&JobRecord> {
        self.job_store.iter().rfind(|j| j.agent_name == agent_name)
    }

    pub fn queue(&self, target: &str) -> serde_json::Value {
        let agents: Vec<serde_json::Value> = if target == "all" {
            self.agent_names
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "agent_name": name,
                        "queue_depth": self.state.queue_depth(name),
                        "active_job_id": self.state.active_job(name),
                    })
                })
                .collect()
        } else {
            vec![serde_json::json!({
                "agent_name": target,
                "queue_depth": self.state.queue_depth(target),
                "active_job_id": self.state.active_job(target),
            })]
        };
        serde_json::json!({
            "target": target,
            "agents": agents,
        })
    }

    pub fn trace(&self, target: &str) -> serde_json::Value {
        let jobs: Vec<&JobRecord> = self
            .job_store
            .iter()
            .filter(|j| j.agent_name == target || target == "all")
            .collect();
        serde_json::json!({
            "target": target,
            "jobs": jobs.iter().map(|j| j.to_record()).collect::<Vec<_>>(),
        })
    }

    pub fn watch(&self, target: &str, start_line: u64) -> serde_json::Value {
        serde_json::json!({
            "target": target,
            "cursor": start_line,
            "lines": [],
            "eof": true,
        })
    }

    pub fn inbox(&self, agent_name: &str) -> serde_json::Value {
        serde_json::json!({
            "agent_name": agent_name,
            "events": [],
            "pending_count": 0,
        })
    }

    pub fn mailbox_head(&self, agent_name: &str) -> serde_json::Value {
        serde_json::json!({
            "agent_name": agent_name,
            "head": null,
        })
    }

    pub fn ack_reply(&self, agent_name: &str, event_id: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "agent_name": agent_name,
            "inbound_event_id": event_id,
            "status": "acked",
        })
    }

    pub fn comms_recover(&self, payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "status": "ok",
            "payload": payload,
        })
    }

    pub fn resubmit(&self, message_id: &str) -> serde_json::Value {
        serde_json::json!({
            "message_id": message_id,
            "status": "resubmitted",
        })
    }

    pub fn retry(&self, target: &str) -> serde_json::Value {
        serde_json::json!({
            "target": target,
            "status": "retried",
        })
    }

    pub fn tick(&mut self) -> Vec<&JobRecord> {
        self.job_store.iter().collect()
    }
}
