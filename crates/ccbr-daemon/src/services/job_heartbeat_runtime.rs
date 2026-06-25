//! Mirrors Python `lib/ccbd/services/job_heartbeat_runtime/`.
//!
//! Wires the ccbr-heartbeat engine into the daemon so running jobs that make
//! no progress are eventually terminalized as heartbeat timeouts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::tick::{
    Dispatcher as TickDispatcher, HeartbeatPolicy, HeartbeatService, HeartbeatState, Job,
    JobSnapshot,
};

/// In-memory store for per-job heartbeat state.
#[derive(Debug, Default, Clone)]
pub struct HeartbeatStateStore {
    states: Arc<Mutex<HashMap<String, HeartbeatState>>>,
}

impl HeartbeatStateStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Service that drives `tick_job_heartbeat` for daemon jobs.
pub struct JobHeartbeatRuntimeService {
    subject_kind: String,
    policy: HeartbeatPolicy,
    store: HeartbeatStateStore,
    terminal_notice_count: Option<u32>,
}

impl JobHeartbeatRuntimeService {
    pub fn new(
        subject_kind: impl Into<String>,
        policy: HeartbeatPolicy,
        terminal_notice_count: Option<u32>,
    ) -> Self {
        Self {
            subject_kind: subject_kind.into(),
            policy,
            store: HeartbeatStateStore::new(),
            terminal_notice_count,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(
            "job_progress",
            HeartbeatPolicy {
                timeout_seconds: 300,
                repeat_interval_seconds: 60,
                max_notices: Some(3),
            },
            Some(3),
        )
    }

    fn key(&self, id: &str) -> String {
        format!("{}:{}", self.subject_kind, id)
    }
}

impl HeartbeatService for JobHeartbeatRuntimeService {
    fn subject_kind(&self) -> String {
        self.subject_kind.clone()
    }

    fn policy(&self) -> &HeartbeatPolicy {
        &self.policy
    }

    fn now(&self) -> String {
        chrono::Utc::now().to_rfc3339()
    }

    fn load(&self, _kind: &str, id: &str) -> crate::tick::Result<Option<HeartbeatState>> {
        Ok(self
            .store
            .states
            .lock()
            .unwrap()
            .get(&self.key(id))
            .cloned())
    }

    fn save(&self, state: HeartbeatState) -> crate::tick::Result<()> {
        self.store
            .states
            .lock()
            .unwrap()
            .insert(self.key(&state.subject_id), state);
        Ok(())
    }

    fn remove(&self, _kind: &str, id: &str) -> crate::tick::Result<()> {
        self.store.states.lock().unwrap().remove(&self.key(id));
        Ok(())
    }

    fn terminal_notice_count(&self) -> Option<u32> {
        self.terminal_notice_count
    }
}

/// Adapter that lets `tick_job_heartbeat` operate on the daemon's
/// `JobDispatcher`.
pub struct JobHeartbeatDispatcherAdapter<'a> {
    pub dispatcher: &'a mut crate::services::dispatcher::JobDispatcher,
}

impl<'a> TickDispatcher for JobHeartbeatDispatcherAdapter<'a> {
    fn get_snapshot(&self, job_id: &str) -> crate::tick::Result<Option<JobSnapshot>> {
        let job = self.dispatcher.get(job_id);
        Ok(job.map(|j| JobSnapshot {
            updated_at: Some(j.updated_at.clone()),
        }))
    }

    fn known_mailbox_targets(&self) -> Vec<String> {
        vec![]
    }

    fn append_event(
        &self,
        _job: &Job,
        _event_type: &str,
        _payload: serde_json::Value,
        _timestamp: &str,
    ) -> crate::tick::Result<()> {
        // Events are not persisted in the Rust dispatcher job store; the
        // timeout terminalization is recorded via complete() and the mailbox.
        Ok(())
    }

    fn complete(&mut self, job_id: &str, decision: serde_json::Value) -> crate::tick::Result<()> {
        let status = decision
            .get("status")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "timeout" => Some(crate::models::api_models::common::JobStatus::Incomplete),
                _ => None,
            })
            .unwrap_or(crate::models::api_models::common::JobStatus::Incomplete);
        self.dispatcher
            .update_job_status(job_id, status, Some(decision));
        Ok(())
    }
}

/// Record a terminal heartbeat timeout in the mailbox layer.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api_models::common::{DeliveryScope, JobStatus};
    use crate::models::api_models::messages::MessageEnvelope;

    fn make_test_job(job_id: &str, agent_name: &str, updated_at: &str) -> Job {
        Job {
            job_id: job_id.into(),
            agent_name: agent_name.into(),
            updated_at: Some(updated_at.into()),
            request: crate::tick::JobRequest {
                from_actor: "user".into(),
            },
        }
    }

    #[test]
    fn heartbeat_timeout_terminalizes_running_job() {
        let mut dispatcher = crate::services::dispatcher::JobDispatcher::new(vec!["agent1".into()]);
        let envelope = MessageEnvelope {
            project_id: "proj-1".into(),
            to_agent: "agent1".into(),
            from_actor: "user".into(),
            body: "hello".into(),
            task_id: None,
            reply_to: None,
            message_type: "ask".into(),
            delivery_scope: DeliveryScope::Single,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        };
        let receipt = dispatcher.submit(&envelope, "agent1", None);
        let job_id = receipt.jobs[0].job_id.clone();
        dispatcher.tick();
        assert_eq!(dispatcher.get(&job_id).unwrap().status, JobStatus::Running);

        let service = JobHeartbeatRuntimeService::new(
            "job_progress",
            HeartbeatPolicy {
                timeout_seconds: 1,
                repeat_interval_seconds: 1,
                max_notices: Some(1),
            },
            Some(1),
        );

        let old = "2026-01-01T00:00:00Z";
        dispatcher.update_job_status(&job_id, JobStatus::Running, None);
        // Force updated_at to an old timestamp so silence is detected.
        if let Some(job) = dispatcher.job_store.iter_mut().find(|j| j.job_id == job_id) {
            job.updated_at = old.into();
        }

        let mut adapter = JobHeartbeatDispatcherAdapter {
            dispatcher: &mut dispatcher,
        };
        let result = crate::tick::tick_job_heartbeat(
            &service,
            &mut adapter,
            &make_test_job(&job_id, "agent1", old),
        );
        assert!(result.is_ok(), "tick should not error: {:?}", result.err());

        let job = dispatcher.get(&job_id).unwrap();
        assert!(
            job.status.is_terminal(),
            "job should be terminalized by heartbeat timeout, got {:?}",
            job.status
        );
    }
}

pub fn record_terminal_timeout(
    dispatcher: &crate::services::dispatcher::JobDispatcher,
    mailbox: &ccbr_mailbox::bureau::MessageBureauFacade,
    job_id: &str,
    finished_at: &str,
) {
    let Some(job) = dispatcher.get(job_id) else {
        return;
    };
    let mailbox_job = crate::adapters::mailbox::to_mailbox_job_record(job);
    let decision = ccbr_mailbox::facade_recording::CompletionDecision {
        terminal: true,
        status: ccbr_mailbox::models::JobStatus::Incomplete,
        reason: Some("heartbeat_timeout".into()),
        reply: "Task stopped after no-progress heartbeat intervals.".into(),
        provider_turn_ref: None,
        diagnostics: serde_json::json!({"heartbeat_timeout": true}),
    };
    let _ = mailbox.record_terminal(&mailbox_job, &decision, finished_at, true, true);
}
