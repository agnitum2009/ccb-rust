use std::collections::HashMap;

use crate::adapters::mailbox::to_mailbox_job_record;
use crate::models::api_models::common::{JobStatus, TargetKind};
use crate::models::api_models::messages::MessageEnvelope;
use crate::models::api_models::receipts::{AcceptedJobReceipt, CancelReceipt, SubmitReceipt};
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

    /// Peek at the next queued job for an agent without removing it.
    pub fn next_queued(&self, agent_name: &str) -> Option<&str> {
        self.queues.get(agent_name)?.first().map(|s| s.as_str())
    }
}

/// Lightweight snapshot of an agent runtime used by `comms_recover` to decide
/// whether a RUNNING job is stale. Mirrors the subset of Python `AgentRuntime`
/// fields consulted by `comms_recover._running_stale_reason`: `state`, `health`,
/// and `pane_state`. Absent entry ≡ Python "registry miss" (`runtime_missing`).
#[derive(Debug, Clone, Default)]
pub struct RuntimeStateSnapshot {
    pub agent_state: String,
    pub health: String,
    pub pane_state: String,
}

/// Mirrors Python `CommsRecoverTarget`.
struct CommsRecoverTarget {
    job_id: String,
    #[allow(dead_code)]
    reply_delivery_job_id: Option<String>,
    block_reason: Option<String>,
}

/// Mirrors Python `CommsRecoverability` (Slice 1 subset).
struct CommsRecoverability {
    recoverable: bool,
    block_reason: Option<String>,
}

/// Mirrors Python `_recover_target_from_payload`.
fn recover_target_from_payload(payload: &serde_json::Value) -> Result<CommsRecoverTarget, String> {
    if let Some(s) = payload.as_str() {
        let job_id = s.trim().to_string();
        if job_id.is_empty() {
            return Err("comms_recover requires job_id".to_string());
        }
        return Ok(CommsRecoverTarget {
            job_id,
            reply_delivery_job_id: None,
            block_reason: None,
        });
    }
    let obj = payload
        .as_object()
        .ok_or_else(|| "comms_recover requires job_id".to_string())?;
    let job_id = obj
        .get("job_id")
        .or_else(|| obj.get("id"))
        .or_else(|| obj.get("target"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if job_id.is_empty() {
        return Err("comms_recover requires job_id".to_string());
    }
    let reply_delivery_job_id = obj
        .get("reply_delivery_job_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let block_reason = obj
        .get("block_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|h| clean_running_hint(&h));
    Ok(CommsRecoverTarget {
        job_id,
        reply_delivery_job_id,
        block_reason,
    })
}

/// Mirrors Python `_clean_running_hint`: returns the hint only if it is one of
/// the recognized running-recovery hints, else `None`.
fn clean_running_hint(value: &str) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    const ALLOWED: &[&str] = &[
        "provider_prompt_idle",
        "provider_prompt_idle_stale",
        "provider_prompt_input_stuck",
        "job_running_stale",
    ];
    if ALLOWED.contains(&text) {
        Some(text.to_string())
    } else {
        None
    }
}

pub struct JobDispatcher {
    pub state: DispatcherState,
    pub job_store: Vec<JobRecord>,
    pub agent_names: Vec<String>,
    mailbox_job_store: Option<ccb_jobs::JobStore>,
    /// Agent runtime snapshots consulted by `comms_recover` for stale-running
    /// detection (mirrors Python `dispatcher._registry`). Empty by default.
    runtime_states: HashMap<String, RuntimeStateSnapshot>,
}

impl JobDispatcher {
    pub fn new(agent_names: Vec<String>) -> Self {
        let state = DispatcherState::new(&agent_names);
        Self {
            state,
            job_store: Vec::new(),
            agent_names,
            mailbox_job_store: None,
            runtime_states: HashMap::new(),
        }
    }

    /// Record an agent runtime snapshot for `comms_recover` stale-running
    /// detection (mirrors Python `registry.upsert(runtime)`).
    pub fn set_runtime_state(
        &mut self,
        agent_name: &str,
        agent_state: &str,
        health: &str,
        pane_state: &str,
    ) {
        self.runtime_states.insert(
            agent_name.to_string(),
            RuntimeStateSnapshot {
                agent_state: agent_state.to_string(),
                health: health.to_string(),
                pane_state: pane_state.to_string(),
            },
        );
    }

    /// Wire the dispatcher to persist job records to the shared mailbox job
    /// store so that `trace` and other mailbox inspection handlers can see
    /// dispatcher job history.
    pub fn with_mailbox_job_store(mut self, store: ccb_jobs::JobStore) -> Self {
        self.mailbox_job_store = Some(store);
        self
    }

    fn persist_job_to_mailbox(&self, job: &JobRecord) {
        let Some(ref store) = self.mailbox_job_store else {
            return;
        };
        let mailbox_job = to_mailbox_job_record(job);
        let _ = store.append(&mailbox_job);
    }

    fn initial_status(&self, agent_name: &str) -> JobStatus {
        // Match Python: a job is queued when the agent already has outstanding
        // work (active or queued); otherwise it is accepted and ready to start.
        if self.state.has_outstanding(agent_name) {
            JobStatus::Queued
        } else {
            JobStatus::Accepted
        }
    }

    pub fn submit(
        &mut self,
        envelope: &MessageEnvelope,
        provider: &str,
        workspace_path: Option<&str>,
    ) -> SubmitReceipt {
        let now = chrono::Utc::now().to_rfc3339();
        let status = self.initial_status(&envelope.to_agent);
        let job_id = format!(
            "job_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        let job = JobRecord {
            job_id: job_id.clone(),
            submission_id: None,
            agent_name: envelope.to_agent.clone(),
            provider: provider.to_string(),
            request: envelope.clone(),
            status,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            workspace_path: workspace_path.map(|s| s.to_string()),
            target_kind: TargetKind::Agent,
            target_name: envelope.to_agent.clone(),
        };
        self.job_store.push(job.clone());
        self.persist_job_to_mailbox(&job);
        self.state.rebuild(&self.job_store);

        SubmitReceipt {
            accepted_at: now.clone(),
            jobs: vec![AcceptedJobReceipt {
                job_id,
                agent_name: envelope.to_agent.clone(),
                status,
                accepted_at: now,
                target_kind: TargetKind::Agent,
                target_name: envelope.to_agent.clone(),
                provider_instance: None,
            }],
            submission_id: None,
        }
    }

    pub fn cancel(&mut self, job_id: &str) -> CancelReceipt {
        let now = chrono::Utc::now().to_rfc3339();
        let mut receipt: Option<CancelReceipt> = None;
        if let Some(job) = self.job_store.iter_mut().find(|j| j.job_id == job_id) {
            match job.status {
                JobStatus::Cancelled => {
                    receipt = Some(CancelReceipt {
                        job_id: job_id.to_string(),
                        agent_name: job.agent_name.clone(),
                        status: JobStatus::Cancelled,
                        cancelled_at: job.updated_at.clone(),
                        target_kind: TargetKind::Agent,
                        target_name: job.agent_name.clone(),
                    });
                }
                JobStatus::Completed | JobStatus::Failed | JobStatus::Incomplete => {
                    // Already terminal; mirror the job's terminal status.
                    let terminal_status = job.status;
                    receipt = Some(CancelReceipt {
                        job_id: job_id.to_string(),
                        agent_name: job.agent_name.clone(),
                        status: terminal_status,
                        cancelled_at: now.clone(),
                        target_kind: TargetKind::Agent,
                        target_name: job.agent_name.clone(),
                    });
                }
                JobStatus::Running => {
                    // Active jobs are marked cancel-requested; the execution layer
                    // will drive the final terminal transition.
                    job.cancel_requested_at = Some(now.clone());
                    job.updated_at = now.clone();
                    receipt = Some(CancelReceipt {
                        job_id: job_id.to_string(),
                        agent_name: job.agent_name.clone(),
                        status: JobStatus::Cancelled,
                        cancelled_at: now.clone(),
                        target_kind: TargetKind::Agent,
                        target_name: job.agent_name.clone(),
                    });
                }
                JobStatus::Accepted | JobStatus::Queued => {
                    job.status = JobStatus::Cancelled;
                    job.cancel_requested_at = Some(now.clone());
                    job.updated_at = now.clone();
                    receipt = Some(CancelReceipt {
                        job_id: job_id.to_string(),
                        agent_name: job.agent_name.clone(),
                        status: JobStatus::Cancelled,
                        cancelled_at: now.clone(),
                        target_kind: TargetKind::Agent,
                        target_name: job.agent_name.clone(),
                    });
                }
            }
        }
        if let Some(job) = self.job_store.iter().find(|j| j.job_id == job_id) {
            self.persist_job_to_mailbox(job);
        }
        if let Some(r) = receipt {
            self.state.rebuild(&self.job_store);
            return r;
        }
        CancelReceipt {
            job_id: job_id.to_string(),
            agent_name: String::new(),
            status: JobStatus::Cancelled,
            cancelled_at: now,
            target_kind: TargetKind::Agent,
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

    pub fn resolve_watch_target(&self, target: &str) -> Option<&JobRecord> {
        let normalized = target.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        if normalized.starts_with("job_") {
            return self.get(&normalized);
        }
        if let Some(job_id) = self.state.active_job(&normalized) {
            return self.get(job_id);
        }
        self.latest_for_agent(&normalized)
    }

    pub fn watch(&self, target: &str, start_line: u64) -> serde_json::Value {
        let job = self.resolve_watch_target(target);
        let job_id = job.map(|j| j.job_id.as_str()).unwrap_or("");
        let agent_name = job.map(|j| j.agent_name.as_str()).unwrap_or("");
        let status = job
            .map(|j| format!("{:?}", j.status).to_lowercase())
            .unwrap_or_default();
        let terminal = job.map(|j| j.status.is_terminal()).unwrap_or(false);
        let reply = job
            .and_then(|j| j.terminal_decision.as_ref())
            .and_then(|d| d.get("reply"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut all_lines: Vec<String> = Vec::new();
        if let Some(job) = job {
            all_lines.push(format!("job {} accepted", job.job_id));
            if job.status == JobStatus::Running || job.status.is_terminal() {
                all_lines.push(format!("job {} started", job.job_id));
            }
            if job.status.is_terminal() {
                let reason = job
                    .terminal_decision
                    .as_ref()
                    .and_then(|d| d.get("reason"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                all_lines.push(format!("job {} {}: {}", job.job_id, status, reason));
            }
        }

        let lines: Vec<String> = all_lines.into_iter().skip(start_line as usize).collect();
        let cursor = start_line + lines.len() as u64;
        serde_json::json!({
            "target": target,
            "job_id": job_id,
            "agent_name": agent_name,
            "status": status,
            "terminal": terminal,
            "reply": reply,
            "cursor": cursor,
            "lines": lines,
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

    /// Communications recovery entrypoint. Mirrors Python
    /// `lib/ccbd/services/dispatcher_runtime/comms_recover.py::comms_recover`.
    ///
    /// **Slice 1 (noop paths).** Determines recoverability and returns `noop`
    /// for non-recoverable jobs: a RUNNING job whose agent runtime is healthy
    /// and has no recognized stale hint → `noop` / `not_recoverable`; an unknown
    /// hint is cleaned to `None` and ignored. Unknown job ids → `noop`.
    /// Recovery actions (terminal retry / stale-running cancel+retry /
    /// reply-delivery) land in later slices.
    pub fn comms_recover(&self, payload: &serde_json::Value) -> serde_json::Value {
        let target = match recover_target_from_payload(payload) {
            Ok(t) => t,
            Err(reason) => {
                return serde_json::json!({ "status": "noop", "noop_reason": reason });
            }
        };
        let source = match self.get(&target.job_id) {
            Some(job) => job,
            None => {
                return serde_json::json!({
                    "status": "noop",
                    "noop_reason": format!("unknown comms job: {}", target.job_id),
                });
            }
        };
        let recoverability =
            self.comms_recoverability_for_job(source, target.block_reason.as_deref());
        let noop_reason: serde_json::Value = if recoverability.recoverable {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(
                recoverability
                    .block_reason
                    .clone()
                    .unwrap_or_else(|| "not_recoverable".to_string()),
            )
        };
        serde_json::json!({
            "job_id": source.job_id,
            "agent_name": source.agent_name,
            "status": if recoverability.recoverable { "pending" } else { "noop" },
            "block_reason": recoverability.block_reason,
            "recoverable": recoverability.recoverable,
            "cancelled_old": null,
            "released_event": null,
            "retried_job": null,
            "next_started": [],
            "noop_reason": noop_reason,
        })
    }

    /// Mirrors Python `comms_recoverability_for_job` (Slice 1 subset: RUNNING
    /// stale detection only). Failed-terminal + reply-delivery retry cases
    /// (which need lineage / `_can_retry_job`) arrive in Slice 2+.
    fn comms_recoverability_for_job(
        &self,
        job: &JobRecord,
        running_hint: Option<&str>,
    ) -> CommsRecoverability {
        if job.status == JobStatus::Running {
            if let Some(reason) = self.running_stale_reason(&job.agent_name, running_hint) {
                return CommsRecoverability {
                    recoverable: true,
                    block_reason: Some(reason),
                };
            }
        }
        CommsRecoverability {
            recoverable: false,
            block_reason: None,
        }
    }

    /// Mirrors Python `_running_stale_reason`. A recognized hint short-circuits
    /// to itself; otherwise the agent runtime snapshot is consulted. Absent
    /// runtime → `runtime_missing` (recoverable).
    fn running_stale_reason(&self, agent_name: &str, running_hint: Option<&str>) -> Option<String> {
        if let Some(hint) = running_hint {
            return Some(hint.to_string());
        }
        let runtime = self.runtime_states.get(agent_name)?;
        const RECOVERABLE_STATES: &[&str] = &["degraded", "failed", "stopped"];
        const STALE_PANE: &[&str] = &["dead", "missing", "lost", "exited"];
        const STALE_HEALTH: &[&str] = &["dead", "failed", "stopped", "unhealthy", "pane-dead"];
        let state = runtime.agent_state.to_lowercase();
        let health = runtime.health.to_lowercase();
        let pane = runtime.pane_state.to_lowercase();
        if RECOVERABLE_STATES.contains(&state.as_str()) {
            if STALE_HEALTH.contains(&health.as_str()) {
                return Some(health.replace('-', "_"));
            }
            if state == "stopped" {
                return Some("runtime_stopped".to_string());
            }
            if state == "failed" {
                return Some("runtime_failed".to_string());
            }
        }
        if STALE_PANE.contains(&pane.as_str()) {
            return Some(format!("pane_{pane}"));
        }
        if STALE_HEALTH.contains(&health.as_str()) {
            return Some(health.replace('-', "_"));
        }
        None
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

    /// Promote one queued job per agent to running when no active job exists.
    pub fn tick(&mut self) -> Vec<&JobRecord> {
        let mut to_start: Vec<String> = Vec::new();
        for agent_name in &self.agent_names {
            if self.state.active_job(agent_name).is_some() {
                continue;
            }
            if let Some(job_id) = self.state.next_queued(agent_name).map(|s| s.to_string()) {
                to_start.push(job_id);
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        for job_id in &to_start {
            let updated = if let Some(job) = self.job_store.iter_mut().find(|j| j.job_id == *job_id)
            {
                job.status = JobStatus::Running;
                job.updated_at = now.clone();
                Some(job.clone())
            } else {
                None
            };
            if let Some(job) = updated {
                self.persist_job_to_mailbox(&job);
            }
        }

        if !to_start.is_empty() {
            self.state.rebuild(&self.job_store);
        }

        to_start
            .iter()
            .filter_map(|id| self.job_store.iter().find(|j| j.job_id == *id))
            .collect()
    }

    /// Mark a job as running once it has been handed to the execution layer.
    pub fn mark_running(&mut self, job_id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let mut updated = None;
        for job in &mut self.job_store {
            if job.job_id == job_id {
                if job.status == JobStatus::Accepted || job.status == JobStatus::Queued {
                    job.status = JobStatus::Running;
                    job.updated_at = now.clone();
                    updated = Some(job.clone());
                }
                break;
            }
        }
        if let Some(job) = updated {
            self.persist_job_to_mailbox(&job);
        }
        self.state.rebuild(&self.job_store);
    }

    /// Update a job's status and optional terminal decision from the execution layer.
    pub fn update_job_status(
        &mut self,
        job_id: &str,
        status: JobStatus,
        terminal_decision: Option<serde_json::Value>,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        let mut updated = None;
        for job in &mut self.job_store {
            if job.job_id == job_id {
                job.status = status;
                job.updated_at = now.clone();
                if let Some(decision) = terminal_decision {
                    job.terminal_decision = Some(decision);
                }
                updated = Some(job.clone());
                break;
            }
        }
        if let Some(job) = updated {
            self.persist_job_to_mailbox(&job);
        }
        self.state.rebuild(&self.job_store);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_envelope(to_agent: &str, body: &str) -> MessageEnvelope {
        MessageEnvelope {
            project_id: "proj-1".into(),
            to_agent: to_agent.into(),
            from_actor: "user".into(),
            body: body.into(),
            task_id: None,
            reply_to: None,
            message_type: "ask".into(),
            delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        }
    }

    #[test]
    fn submit_creates_a_job_in_the_queue() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        assert_eq!(receipt.jobs.len(), 1);
        let job_id = &receipt.jobs[0].job_id;
        assert_eq!(receipt.jobs[0].status, JobStatus::Accepted);
        assert_eq!(dispatcher.state.queue_depth("claude"), 1);
        assert!(dispatcher.state.active_job("claude").is_none());
        let job = dispatcher.get(job_id).unwrap();
        assert_eq!(job.status, JobStatus::Accepted);
    }

    #[test]
    fn submit_second_job_while_first_is_pending_creates_queued_job() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let first = dispatcher.submit(&test_envelope("claude", "first"), "claude", None);
        let second = dispatcher.submit(&test_envelope("claude", "second"), "claude", None);
        assert_eq!(first.jobs[0].status, JobStatus::Accepted);
        assert_eq!(second.jobs[0].status, JobStatus::Queued);
        assert_eq!(dispatcher.state.queue_depth("claude"), 2);
        assert!(dispatcher.state.active_job("claude").is_none());
    }

    #[test]
    fn tick_promotes_queued_job_to_running() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();

        let started = dispatcher.tick();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].job_id, job_id);

        let job = dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Running);
        assert_eq!(dispatcher.state.active_job("claude"), Some(job_id.as_str()));
        assert_eq!(dispatcher.state.queue_depth("claude"), 0);
    }

    #[test]
    fn tick_only_starts_one_job_per_agent() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let first = dispatcher.submit(&test_envelope("claude", "first"), "claude", None);
        let second = dispatcher.submit(&test_envelope("claude", "second"), "claude", None);
        let first_id = first.jobs[0].job_id.clone();
        let second_id = second.jobs[0].job_id.clone();

        let started = dispatcher.tick();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].job_id, first_id);

        assert_eq!(
            dispatcher.get(&first_id).unwrap().status,
            JobStatus::Running
        );
        assert_eq!(
            dispatcher.get(&second_id).unwrap().status,
            JobStatus::Queued
        );
        assert_eq!(
            dispatcher.state.active_job("claude"),
            Some(first_id.as_str())
        );
        assert_eq!(dispatcher.state.queue_depth("claude"), 1);

        // A second tick must not start another job while the first is still running.
        let started_again = dispatcher.tick();
        assert!(started_again.is_empty());
        assert_eq!(
            dispatcher.get(&second_id).unwrap().status,
            JobStatus::Queued
        );
    }

    #[test]
    fn cancel_queued_job_marks_it_cancelled() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();

        let cancel_receipt = dispatcher.cancel(&job_id);
        assert_eq!(cancel_receipt.status, JobStatus::Cancelled);

        let job = dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.cancel_requested_at.is_some());
        assert_eq!(dispatcher.state.queue_depth("claude"), 0);
        assert!(dispatcher.state.active_job("claude").is_none());
    }

    #[test]
    fn cancel_running_job_marks_cancel_requested() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();
        dispatcher.tick();

        let job_before = dispatcher.get(&job_id).unwrap();
        assert_eq!(job_before.status, JobStatus::Running);
        assert!(job_before.cancel_requested_at.is_none());

        let cancel_receipt = dispatcher.cancel(&job_id);
        assert_eq!(cancel_receipt.status, JobStatus::Cancelled);

        let job = dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Running);
        assert!(job.cancel_requested_at.is_some());
        assert_eq!(dispatcher.state.active_job("claude"), Some(job_id.as_str()));
    }

    #[test]
    fn terminal_update_clears_active_job() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();
        dispatcher.tick();
        assert_eq!(dispatcher.state.active_job("claude"), Some(job_id.as_str()));

        dispatcher.update_job_status(
            &job_id,
            JobStatus::Completed,
            Some(serde_json::json!({"terminal": true})),
        );

        let job = dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert!(job.terminal_decision.is_some());
        assert!(dispatcher.state.active_job("claude").is_none());
        assert_eq!(dispatcher.state.queue_depth("claude"), 0);
    }

    #[test]
    fn terminal_update_allows_next_queued_job_to_start() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let first = dispatcher.submit(&test_envelope("claude", "first"), "claude", None);
        let second = dispatcher.submit(&test_envelope("claude", "second"), "claude", None);
        let first_id = first.jobs[0].job_id.clone();
        let second_id = second.jobs[0].job_id.clone();

        dispatcher.tick();
        assert_eq!(
            dispatcher.state.active_job("claude"),
            Some(first_id.as_str())
        );

        dispatcher.update_job_status(&first_id, JobStatus::Completed, None);
        assert!(dispatcher.state.active_job("claude").is_none());

        let started = dispatcher.tick();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].job_id, second_id);
        assert_eq!(
            dispatcher.get(&second_id).unwrap().status,
            JobStatus::Running
        );
        assert_eq!(
            dispatcher.state.active_job("claude"),
            Some(second_id.as_str())
        );
    }

    #[test]
    fn cancel_completed_job_reports_terminal_status() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();
        dispatcher.tick();
        dispatcher.update_job_status(&job_id, JobStatus::Completed, None);

        let cancel_receipt = dispatcher.cancel(&job_id);
        assert_eq!(cancel_receipt.status, JobStatus::Completed);
        assert_eq!(
            dispatcher.get(&job_id).unwrap().status,
            JobStatus::Completed
        );
    }
}
