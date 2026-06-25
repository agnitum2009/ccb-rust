use std::collections::HashMap;
use std::sync::Arc;

use ccbr_completion::models::{CompletionDecision, CompletionStatus};
use ccbr_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade};
use ccbr_mailbox::facade_recording::CompletionDecision as MailboxCompletionDecision;
use ccbr_mailbox::models::{
    AttemptRecord, AttemptState, CallbackEdgeRecord, CallbackEdgeState, MessageState,
    ReplyTerminalStatus,
};
use ccbr_mailbox::stores::{AttemptStore, CallbackEdgeChanges};
use ccbr_storage::text_artifacts::{maybe_spill_text, TEXT_ARTIFACT_SPILL_BYTES};

use crate::adapters::mailbox::to_mailbox_job_record;
use crate::models::api_models::common::{DeliveryScope, JobStatus, TargetKind};
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
    mailbox_job_store: Option<ccbr_jobs::JobStore>,
    /// Agent runtime snapshots consulted by `comms_recover` for stale-running
    /// detection (mirrors Python `dispatcher._registry`). Empty by default.
    runtime_states: HashMap<String, RuntimeStateSnapshot>,
    /// Attempt lineage store (mirrors Python `_message_bureau_control._attempt_store`).
    /// When wired, `submit`/`retry` record attempts so `comms_recover` can detect
    /// retry lineage / `already_retried`.
    attempt_store: Option<AttemptStore>,
    /// Reply-delivery lineage: source job id → delivery job ids (mirrors the
    /// reply-delivery job chain Python creates via `prepare_reply_deliveries`).
    reply_deliveries: HashMap<String, Vec<String>>,
    /// Real mailbox control service (mirrors Python `_message_bureau_control`).
    /// When wired, comms_recover uses its kernel/inbound/attempt stores for
    /// terminal-retry head release + lineage (tests 8/12). When absent, the
    /// simplified `attempt_store` path is used (tests 1-7, 9-11).
    mailbox_control: Option<MessageBureauControlService>,
    /// Real mailbox facade (mirrors Python `_message_bureau_control`'s facade).
    /// Wrapped in `Arc` so callback helpers can use it while also mutating the
    /// dispatcher state.
    mailbox: Option<Arc<MessageBureauFacade>>,
    /// Project path layout, used for callback continuation text artifact spill.
    layout: Option<ccbr_storage::paths::PathLayout>,
    /// Configurable clock for deterministic tests. Defaults to `Utc::now()`.
    clock: Arc<dyn Fn() -> String + Send + Sync>,
    /// Callback timeout in seconds (mirrors Python `config.callback_timeout_s`).
    callback_timeout_s: f64,
    /// Maximum callback chain depth (mirrors Python `config.max_callback_depth`).
    max_callback_depth: u32,
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
            attempt_store: None,
            reply_deliveries: HashMap::new(),
            mailbox_control: None,
            mailbox: None,
            layout: None,
            clock: Arc::new(|| chrono::Utc::now().to_rfc3339()),
            callback_timeout_s: 30.0 * 60.0,
            max_callback_depth: 5,
        }
    }

    fn now(&self) -> String {
        (self.clock)()
    }

    pub fn with_clock(mut self, clock: Arc<dyn Fn() -> String + Send + Sync>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_callback_timeout_s(mut self, seconds: f64) -> Self {
        self.callback_timeout_s = seconds.max(0.0);
        self
    }

    pub fn with_max_callback_depth(mut self, depth: u32) -> Self {
        self.max_callback_depth = depth.max(1);
        self
    }

    pub fn with_layout(mut self, layout: ccbr_storage::paths::PathLayout) -> Self {
        self.layout = Some(layout);
        self
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

    /// Persist all non-terminal jobs so a restarted daemon can restore running
    /// job memory.  This closes the S3.3 daemon-restart job-continuity gap.
    pub fn persist_running_jobs(&self, path: &camino::Utf8Path) -> Result<(), String> {
        let running: Vec<&JobRecord> = self
            .job_store
            .iter()
            .filter(|j| !j.status.is_terminal())
            .collect();
        ccbr_storage::json::JsonStore::new()
            .save(path, &running)
            .map_err(|e| format!("failed to persist running jobs: {e}"))
    }

    /// Restore non-terminal jobs from a previous daemon instance.  Jobs whose
    /// agent is no longer configured are dropped.  Running jobs are left in the
    /// dispatcher state so heartbeat/comms_recover can drive them to completion.
    pub fn restore_running_jobs(&mut self, path: &camino::Utf8Path) {
        let loaded: Vec<JobRecord> = ccbr_storage::json::JsonStore::new()
            .load(path)
            .unwrap_or_default();
        let valid: Vec<JobRecord> = loaded
            .into_iter()
            .filter(|j| self.agent_names.contains(&j.agent_name) && !j.status.is_terminal())
            .collect();
        if !valid.is_empty() {
            self.job_store = valid;
            self.state.rebuild(&self.job_store);
        }
    }

    /// Wire the dispatcher to persist job records to the shared mailbox job
    /// store so that `trace` and other mailbox inspection handlers can see
    /// dispatcher job history.
    pub fn with_mailbox_job_store(mut self, store: ccbr_jobs::JobStore) -> Self {
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

    /// Wire the attempt lineage store (mirrors Python `_attempt_store`). When
    /// set, `submit` records an initial attempt and `retry` records the retry
    /// chain, enabling `comms_recover` lineage / `already_retried` detection.
    pub fn with_attempt_store(mut self, store: AttemptStore) -> Self {
        self.attempt_store = Some(store);
        self
    }

    /// Wire the real mailbox control service (mirrors Python
    /// `_message_bureau_control`). When set, comms_recover uses its
    /// kernel/inbound/attempt stores for terminal-retry head release + lineage.
    pub fn with_mailbox_control(mut self, control: MessageBureauControlService) -> Self {
        self.mailbox_control = Some(control);
        self
    }

    /// Wire the real mailbox facade so `submit` records full message+inbound+
    /// attempt state via `record_submission` (mirrors Python JobDispatcher
    /// owning the message bureau). Pair with `with_mailbox_control`.
    pub fn with_mailbox(mut self, facade: MessageBureauFacade) -> Self {
        self.mailbox = Some(Arc::new(facade));
        self
    }

    /// Unified attempt-lineage store: the real mailbox attempt store when a
    /// control service is wired, else the simplified `attempt_store`.
    fn lineage_store(&self) -> Option<&AttemptStore> {
        if let Some(control) = &self.mailbox_control {
            Some(control.attempt_store())
        } else {
            self.attempt_store.as_ref()
        }
    }

    /// Append an attempt record linking `job` to `message_id` with the given
    /// retry index and state (mirrors Python attempt recording).
    fn record_attempt(
        &self,
        job: &JobRecord,
        message_id: &str,
        retry_index: u32,
        state: AttemptState,
    ) {
        let Some(store) = self.lineage_store() else {
            return;
        };
        let now = chrono::Utc::now().to_rfc3339();
        let attempt = AttemptRecord {
            attempt_id: format!(
                "att_{}",
                &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
            ),
            message_id: message_id.to_string(),
            agent_name: job.agent_name.clone(),
            provider: job.provider.clone(),
            job_id: job.job_id.clone(),
            retry_index,
            health_snapshot_ref: None,
            started_at: now.clone(),
            updated_at: now,
            attempt_state: state,
        };
        let _ = store.append(&attempt);
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
        let now = self.now();
        if let Err(reason) = self.validate_callback_request(envelope) {
            panic!("callback validation failed: {reason}");
        }
        let status = self.initial_status(&envelope.to_agent);
        let job_id = self.new_id("job");
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
        // Record attempt lineage. When the real mailbox facade is wired, record
        // the full message+inbound+attempt via record_submission (mirrors Python
        // JobDispatcher owning the message bureau); otherwise record a
        // simplified attempt for lineage.
        let message_id = if let Some(facade) = &self.mailbox {
            let mb_env = crate::adapters::mailbox::to_mailbox_envelope(envelope);
            let mb_job = crate::adapters::mailbox::to_mailbox_job_record(&job);
            facade
                .record_submission(&mb_env, &[mb_job], None, &now, None)
                .unwrap_or_else(|| self.new_id("msg"))
        } else {
            let message_id = self.new_id("msg");
            self.record_attempt(&job, &message_id, 0, AttemptState::Running);
            message_id
        };
        let _ = self.register_callback_edge(envelope, &job, &message_id, &now);

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

    /// Cancel a job, mirroring Python `cancel_job` semantics with one Rust-only
    /// UX adjustment: cancelling an unknown job is treated as idempotent success.
    ///
    /// - Unknown job -> returns a success receipt (idempotent CLI cancel).
    /// - Already cancelled -> returns the existing receipt.
    /// - Terminal job -> error.
    /// - Running job -> immediately terminalized as Cancelled.
    /// - Accepted/Queued -> marked Cancelled.
    pub fn cancel(&mut self, job_id: &str) -> Result<CancelReceipt, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let job = match self.job_store.iter().find(|j| j.job_id == job_id) {
            Some(j) => j,
            None => {
                return Ok(CancelReceipt {
                    job_id: job_id.to_string(),
                    agent_name: String::new(),
                    status: JobStatus::Cancelled,
                    cancelled_at: now,
                    target_kind: TargetKind::Agent,
                    target_name: String::new(),
                });
            }
        };

        match job.status {
            JobStatus::Cancelled => {
                return Ok(CancelReceipt {
                    job_id: job_id.to_string(),
                    agent_name: job.agent_name.clone(),
                    status: JobStatus::Cancelled,
                    cancelled_at: job.updated_at.clone(),
                    target_kind: job.target_kind,
                    target_name: job.target_name.clone(),
                });
            }
            JobStatus::Completed | JobStatus::Failed | JobStatus::Incomplete => {
                return Err(format!("job is already terminal: {:?}", job.status));
            }
            _ => {}
        }

        let mut job = job.clone();
        job.cancel_requested_at = Some(now.clone());
        job.updated_at = now.clone();

        if job.status == JobStatus::Running {
            // Mid-run cancellation is terminalized immediately, matching Python.
            job.status = JobStatus::Cancelled;
            let decision = CompletionDecision {
                terminal: true,
                status: CompletionStatus::Cancelled,
                reason: Some("cancel_info".into()),
                confidence: Some(ccbr_completion::models::CompletionConfidence::Degraded),
                reply: String::new(),
                anchor_seen: false,
                reply_started: false,
                reply_stable: false,
                provider_turn_ref: None,
                source_cursor: None,
                finished_at: Some(now.clone()),
                diagnostics: Default::default(),
            };
            job.terminal_decision = Some(decision.to_record());
        } else {
            // Accepted / Queued -> cancelled directly.
            job.status = JobStatus::Cancelled;
        }

        self.job_store.retain(|j| j.job_id != job_id);
        self.job_store.push(job.clone());
        self.state.rebuild(&self.job_store);
        self.persist_job_to_mailbox(&job);

        Ok(CancelReceipt {
            job_id: job_id.to_string(),
            agent_name: job.agent_name.clone(),
            status: JobStatus::Cancelled,
            cancelled_at: now.clone(),
            target_kind: job.target_kind,
            target_name: job.target_name.clone(),
        })
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

    // =========================================================================
    // Callback subsystem (mirrors Python callbacks.py)
    // =========================================================================

    /// Return `true` if the request explicitly asks for callback routing.
    fn request_callback_route(envelope: &MessageEnvelope) -> bool {
        envelope
            .route_options
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().eq_ignore_ascii_case("callback"))
            .unwrap_or(false)
    }

    /// Validate a plain nested `ask` from an active parent: it must either use
    /// callback routing or silence-on-success.
    fn validate_nested_ask_request(&self, envelope: &MessageEnvelope) -> Result<(), String> {
        if Self::request_callback_route(envelope) {
            return Ok(());
        }
        if Self::is_plain_ask(envelope) && !envelope.silence_on_success {
            let parent = self.active_parent_job(&envelope.from_actor);
            if parent.is_some() {
                return Err(
                    "plain ask from an active CCBR task requires --callback when the child result is needed, or --silence for independent fire-and-forget work".into(),
                );
            }
        }
        Ok(())
    }

    fn is_plain_ask(envelope: &MessageEnvelope) -> bool {
        envelope.message_type.trim().eq_ignore_ascii_case("ask")
    }

    /// Validate a callback request before accepting it.
    fn validate_callback_request(&self, envelope: &MessageEnvelope) -> Result<(), String> {
        if !Self::request_callback_route(envelope) {
            return self.validate_nested_ask_request(envelope);
        }
        if self.mailbox.is_none() {
            return Err("ask --callback requires message bureau support".into());
        }
        if envelope.delivery_scope != DeliveryScope::Single {
            return Err("ask --callback supports exactly one target agent".into());
        }
        let parent = self
            .active_parent_job(&envelope.from_actor)
            .ok_or_else(|| {
                "ask --callback requires an active parent job for the sender".to_string()
            })?;
        if self.message_for_job(parent).is_none() {
            return Err("ask --callback could not resolve parent message".into());
        }
        if let Some(facade) = &self.mailbox {
            if facade
                .callback_edge_for_parent_job(&parent.job_id)
                .is_some()
            {
                return Err("ask --callback allows one outstanding callback per parent job".into());
            }
        }
        self.validate_callback_chain(parent, &envelope.to_agent)
    }

    /// Create and persist a callback edge when a callback-routed child is submitted.
    fn register_callback_edge(
        &self,
        request: &MessageEnvelope,
        child: &JobRecord,
        message_id: &str,
        accepted_at: &str,
    ) -> Result<(), String> {
        if !Self::request_callback_route(request) {
            return Ok(());
        }
        let parent = self.active_parent_job(&request.from_actor).ok_or_else(|| {
            "ask --callback requires an active parent job for the sender".to_string()
        })?;
        let parent_message = self
            .message_for_job(parent)
            .ok_or_else(|| "ask --callback could not resolve parent message".to_string())?;
        let edge = CallbackEdgeRecord {
            edge_id: self.new_id("cb"),
            parent_job_id: parent.job_id.clone(),
            parent_message_id: parent_message.message_id.clone(),
            parent_agent: parent.agent_name.clone(),
            child_job_id: child.job_id.clone(),
            child_message_id: message_id.to_string(),
            callback_target_agent: parent.agent_name.clone(),
            original_caller: parent.request.from_actor.clone(),
            original_task_id: parent
                .request
                .task_id
                .clone()
                .or(Some(parent_message.message_id.clone())),
            state: CallbackEdgeState::Pending,
            child_reply_id: None,
            child_status: None,
            continuation_job_id: None,
            continuation_message_id: None,
            timeout_at: Some(self.callback_timeout_at(accepted_at)),
            created_at: accepted_at.to_string(),
            updated_at: accepted_at.to_string(),
            diagnostics: serde_json::json!({
                "route_mode": "callback",
                "child_agent": child.agent_name,
                "parent_body": Self::callback_body_summary(&parent.request),
                "child_body": Self::callback_body_summary(request),
                "artifact_request": request.route_options.get("artifact_request").and_then(|v| v.as_bool()).unwrap_or(false),
                "artifact_reply": request.route_options.get("artifact_reply").and_then(|v| v.as_bool()).unwrap_or(false),
            }),
        };
        if let Some(facade) = &self.mailbox {
            let _ = facade.record_callback_edge(&edge);
        }
        Ok(())
    }

    /// Return the edge for which `job` is the parent, if any.
    fn delegated_parent_edge(&self, job: &JobRecord) -> Option<CallbackEdgeRecord> {
        let facade = self.mailbox.clone()?;
        facade.callback_edge_for_parent_job(&job.job_id)
    }

    /// Return the edge for which `job` is the child, if any.
    fn callback_child_edge(&self, job: &JobRecord) -> Option<CallbackEdgeRecord> {
        let facade = self.mailbox.clone()?;
        if let Some(edge) = facade.callback_edge_for_child_job(&job.job_id) {
            return Some(edge);
        }
        let message = self.message_for_job(job)?;
        facade.callback_edge_for_child_message(&message.message_id)
    }

    /// Called when a callback child completes: transition the edge and submit a
    /// continuation job that resumes the parent task with the child result.
    fn submit_callback_continuation(
        &mut self,
        edge: &CallbackEdgeRecord,
        child_job: &JobRecord,
        child_reply_id: Option<&str>,
        decision: &CompletionDecision,
        finished_at: &str,
    ) -> Option<CallbackEdgeRecord> {
        let facade = self.mailbox.clone()?;
        let latest = facade
            .callback_edge_for_child_job(&child_job.job_id)
            .as_ref()
            .unwrap_or(edge)
            .clone();
        if Self::terminal_callback_state(&latest.state) {
            return Some(latest);
        }
        if let Some(existing) = self.existing_continuation_job(&latest) {
            let persisted_reply = self.latest_child_reply(&latest);
            let state = if existing.status.is_terminal() {
                CallbackEdgeState::Done
            } else {
                CallbackEdgeState::ContinuationSubmitted
            };
            let child_reply_id = child_reply_id
                .map(|s| s.to_string())
                .or(latest.child_reply_id.clone())
                .or(persisted_reply.as_ref().map(|r| r.reply_id.clone()));
            let updated = facade.update_callback_edge(
                &latest,
                CallbackEdgeChanges {
                    state: Some(state),
                    child_reply_id,
                    child_status: Some(format!("{:?}", existing.status).to_lowercase()),
                    continuation_job_id: Some(existing.job_id.clone()),
                    continuation_message_id: Some(latest.parent_message_id.clone()),
                    timeout_at: Some(None),
                    diagnostics: None,
                    updated_at: Some(finished_at.to_string()),
                },
            );
            return Some(updated);
        }
        let updated = facade.update_callback_edge(
            &latest,
            CallbackEdgeChanges {
                state: Some(CallbackEdgeState::ChildCompleted),
                child_reply_id: child_reply_id.map(|s| s.to_string()),
                child_status: Some(format!("{:?}", child_job.status).to_lowercase()),
                continuation_job_id: None,
                continuation_message_id: None,
                timeout_at: Some(None),
                diagnostics: None,
                updated_at: Some(finished_at.to_string()),
            },
        );
        let (continuation_job_id, continuation_message_id) =
            match self.submit_continuation_job(&updated, child_job, decision, finished_at) {
                Ok(ids) => ids,
                Err(reason) => {
                    return self.fail_callback_edge(
                        &updated,
                        "callback_continuation_submit_failed",
                        &reason,
                        finished_at,
                        CallbackEdgeState::Failed,
                    );
                }
            };
        let final_edge = facade.update_callback_edge(
            &updated,
            CallbackEdgeChanges {
                state: Some(CallbackEdgeState::ContinuationSubmitted),
                child_reply_id: updated.child_reply_id.clone(),
                child_status: updated.child_status.clone(),
                continuation_job_id: Some(continuation_job_id),
                continuation_message_id: Some(continuation_message_id),
                timeout_at: Some(None),
                diagnostics: None,
                updated_at: Some(finished_at.to_string()),
            },
        );
        Some(final_edge)
    }

    /// Submit a continuation job that resumes the parent task.
    fn submit_continuation_job(
        &mut self,
        edge: &CallbackEdgeRecord,
        child_job: &JobRecord,
        decision: &CompletionDecision,
        accepted_at: &str,
    ) -> Result<(String, String), String> {
        if !self.agent_names.contains(&edge.callback_target_agent) {
            return Err(format!(
                "callback target agent {} not available",
                edge.callback_target_agent
            ));
        }
        let request = self.continuation_request(edge, child_job, decision)?;
        let parent_message_id = edge.parent_message_id.clone();
        let status = self.initial_status(&request.to_agent);
        let job_id = self.new_id("job");
        let job = JobRecord {
            job_id: job_id.clone(),
            submission_id: None,
            agent_name: request.to_agent.clone(),
            provider: child_job.provider.clone(),
            request: request.clone(),
            status,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: accepted_at.to_string(),
            updated_at: accepted_at.to_string(),
            workspace_path: None,
            target_kind: TargetKind::Agent,
            target_name: request.to_agent.clone(),
        };
        self.job_store.push(job.clone());
        self.persist_job_to_mailbox(&job);
        self.state.rebuild(&self.job_store);
        if let Some(facade) = &self.mailbox {
            let mb_job = crate::adapters::mailbox::to_mailbox_job_record(&job);
            let _ = facade.record_retry_attempt(&parent_message_id, &mb_job, accepted_at);
        }
        Ok((job_id, parent_message_id))
    }

    /// Build the `MessageEnvelope` for a callback continuation.
    fn continuation_request(
        &self,
        edge: &CallbackEdgeRecord,
        child_job: &JobRecord,
        decision: &CompletionDecision,
    ) -> Result<MessageEnvelope, String> {
        let body = self.continuation_body(edge, child_job, decision);
        let prefix = format!(
            "CCBR callback continuation {} is larger than 4 KiB and was stored as an artifact.",
            edge.edge_id
        );
        let artifact_reply = edge
            .diagnostics
            .get("artifact_reply")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let (body, body_artifact) = if let Some(layout) = self.mailbox_layout() {
            let threshold = if artifact_reply {
                Some(1)
            } else {
                Some(TEXT_ARTIFACT_SPILL_BYTES)
            };
            let (b, art) = maybe_spill_text(
                layout,
                &body,
                "callback-continuation",
                &edge.edge_id,
                &prefix,
                threshold,
                None,
                Some(&self.now()),
            )
            .map_err(|e| format!("failed to spill callback continuation: {e}"))?;
            (
                b,
                art.map(|a| serde_json::to_value(a.to_record()).unwrap_or_default()),
            )
        } else {
            (body, None)
        };
        Ok(MessageEnvelope {
            project_id: child_job.request.project_id.clone(),
            to_agent: edge.callback_target_agent.clone(),
            from_actor: edge.original_caller.clone(),
            body,
            task_id: edge.original_task_id.clone(),
            reply_to: Some(edge.parent_message_id.clone()),
            message_type: "callback_continuation".to_string(),
            delivery_scope: DeliveryScope::Single,
            silence_on_success: false,
            route_options: serde_json::json!({
                "mode": "callback_continuation",
                "callback_edge_id": edge.edge_id,
                "callback_parent_job_id": edge.parent_job_id,
                "callback_child_job_id": edge.child_job_id,
                "callback_child_message_id": edge.child_message_id,
            }),
            body_artifact,
        })
    }

    fn continuation_body(
        &self,
        edge: &CallbackEdgeRecord,
        child_job: &JobRecord,
        decision: &CompletionDecision,
    ) -> String {
        let original = edge
            .diagnostics
            .get("parent_body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_end();
        let child_task = edge
            .diagnostics
            .get("child_body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_end();
        let child_reply = self.reply_summary(decision);
        let mut parts = vec![
            "CCBR callback continuation.".to_string(),
            String::new(),
            format!("Original caller: {}", edge.original_caller),
            format!("Parent job: {}", edge.parent_job_id),
            format!("Child job: {}", child_job.job_id),
            format!("Child agent: {}", child_job.agent_name),
            format!(
                "Child status: {}",
                format!("{:?}", child_job.status).to_lowercase()
            ),
        ];
        if !original.is_empty() {
            parts.push(String::new());
            parts.push("Original task context:".to_string());
            parts.push(original.to_string());
        }
        if !child_task.is_empty() {
            parts.push(String::new());
            parts.push("Delegated child task:".to_string());
            parts.push(child_task.to_string());
        }
        parts.push(String::new());
        parts.push("Child result:".to_string());
        parts.push(child_reply);
        parts.push(String::new());
        parts.push("Continue the original task using the child result. Reply to the original caller with the final result.".to_string());
        parts.join("\n")
    }

    /// Repair pending/child-completed edges that lost their continuation job
    /// across a crash. Called from `tick`.
    fn repair_callback_edges(&mut self) -> Vec<CallbackEdgeRecord> {
        let facade = match self.mailbox.clone() {
            Some(f) => f,
            None => return Vec::new(),
        };
        let mut repaired = Vec::new();
        let pending = facade.pending_callback_edges();
        for edge in pending {
            let latest = facade
                .callback_edge(&edge.edge_id)
                .as_ref()
                .unwrap_or(&edge)
                .clone();
            if Self::terminal_callback_state(&latest.state) {
                continue;
            }
            if self.callback_edge_expired(&latest) {
                continue;
            }
            if latest.continuation_job_id.is_some() {
                continue;
            }
            if let Some(existing) = self.existing_continuation_job(&latest) {
                let reply = self.latest_child_reply(&latest);
                let reply_job = reply.as_ref().and_then(|r| self.job_for_reply(r));
                let updated = facade.update_callback_edge(
                    &latest,
                    CallbackEdgeChanges {
                        state: Some(CallbackEdgeState::ContinuationSubmitted),
                        child_reply_id: latest
                            .child_reply_id
                            .clone()
                            .or(reply.as_ref().map(|r| r.reply_id.clone())),
                        child_status: latest.child_status.clone().or(reply_job
                            .as_ref()
                            .map(|j| format!("{:?}", j.status).to_lowercase())),
                        continuation_job_id: Some(existing.job_id.clone()),
                        continuation_message_id: Some(latest.parent_message_id.clone()),
                        timeout_at: Some(None),
                        diagnostics: None,
                        updated_at: Some(if latest.updated_at.is_empty() {
                            self.now()
                        } else {
                            latest.updated_at.clone()
                        }),
                    },
                );
                repaired.push(updated);
                continue;
            }
            let reply = self.latest_child_reply(&latest);
            let child_job = reply
                .as_ref()
                .and_then(|r| self.job_for_reply(r))
                .or_else(|| self.get(&latest.child_job_id).cloned());
            let decision = reply.as_ref().and_then(|r| {
                child_job
                    .as_ref()
                    .map(|j| self.decision_from_reply(r, j, &latest.updated_at))
            });
            if reply.is_none() || child_job.is_none() || decision.is_none() {
                continue;
            }
            let reply = reply.unwrap();
            let child_job = child_job.unwrap();
            let decision = decision.unwrap();
            let finished_at = if !latest.updated_at.is_empty() {
                latest.updated_at.clone()
            } else if !reply.finished_at.is_empty() {
                reply.finished_at.clone()
            } else {
                self.now()
            };
            if let Some(updated) = self.submit_callback_continuation(
                &latest,
                &child_job,
                Some(&reply.reply_id),
                &decision,
                &finished_at,
            ) {
                repaired.push(updated);
            }
        }
        repaired
    }

    /// Sweep expired pending edges and fail them. Called from `tick`.
    fn sweep_callback_timeouts(&mut self) -> Vec<CallbackEdgeRecord> {
        let facade = match self.mailbox.clone() {
            Some(f) => f,
            None => return Vec::new(),
        };
        let pending = facade.pending_callback_edges();
        if pending.is_empty() {
            return Vec::new();
        }
        let mut expired = Vec::new();
        for edge in pending {
            let latest = facade
                .callback_edge(&edge.edge_id)
                .as_ref()
                .unwrap_or(&edge)
                .clone();
            if Self::terminal_callback_state(&latest.state) {
                continue;
            }
            if !self.callback_edge_expired(&latest) {
                continue;
            }
            if let Some(failed) = self.fail_callback_edge(
                &latest,
                "callback_timeout",
                "callback child did not produce a continuation before timeout",
                &self.now(),
                CallbackEdgeState::TimedOut,
            ) {
                expired.push(failed);
            }
        }
        expired
    }

    fn fail_callback_edge(
        &self,
        edge: &CallbackEdgeRecord,
        reason: &str,
        detail: &str,
        updated_at: &str,
        state: CallbackEdgeState,
    ) -> Option<CallbackEdgeRecord> {
        let facade = self.mailbox.clone()?;
        let latest = facade
            .callback_edge(&edge.edge_id)
            .as_ref()
            .unwrap_or(edge)
            .clone();
        if Self::terminal_callback_state(&latest.state) {
            return Some(latest);
        }
        let mut diagnostics = latest.diagnostics.clone();
        if let Some(obj) = diagnostics.as_object_mut() {
            obj.insert("failure_reason".to_string(), reason.into());
            obj.insert("failure_detail".to_string(), detail.into());
        }
        let failed = facade.update_callback_edge(
            &latest,
            CallbackEdgeChanges {
                state: Some(state),
                child_reply_id: latest.child_reply_id.clone(),
                child_status: latest
                    .child_status
                    .clone()
                    .or(Some(format!("{:?}", state).to_lowercase())),
                continuation_job_id: None,
                continuation_message_id: None,
                timeout_at: Some(None),
                diagnostics: Some(diagnostics),
                updated_at: Some(updated_at.to_string()),
            },
        );
        self.record_callback_failure_notice(&failed, reason, detail, updated_at);
        Some(failed)
    }

    fn record_callback_failure_notice(
        &self,
        edge: &CallbackEdgeRecord,
        reason: &str,
        detail: &str,
        updated_at: &str,
    ) {
        let facade = match &self.mailbox {
            Some(f) => f,
            None => return,
        };
        let reply = self.callback_failure_reply(edge, reason, detail);
        let diagnostics = serde_json::json!({
            "notice": true,
            "callback_edge_id": edge.edge_id,
            "callback_failure": true,
            "reason": reason,
            "detail": detail,
        });
        facade.set_message_state(&edge.parent_message_id, MessageState::Failed, updated_at);
        if let Some(parent_job) = self.get(&edge.parent_job_id).cloned() {
            facade.record_notice(
                &crate::adapters::mailbox::to_mailbox_job_record(&parent_job),
                &reply,
                Some(diagnostics.clone()),
                updated_at,
                ReplyTerminalStatus::Failed,
                Some(&edge.original_caller),
            );
        }
    }

    fn callback_failure_reply(
        &self,
        edge: &CallbackEdgeRecord,
        reason: &str,
        detail: &str,
    ) -> String {
        let suffix = if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        };
        format!(
            "CCBR callback failed for delegated task {} while continuing parent job {}. Reason: {}{}",
            edge.child_job_id, edge.parent_job_id, reason, suffix
        )
    }

    /// Mark a callback edge DONE when its continuation job completes.
    fn mark_callback_done(&self, job: &JobRecord, finished_at: &str) {
        let edge_id = job
            .request
            .route_options
            .get("callback_edge_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if edge_id.is_empty() {
            return;
        }
        let facade = match &self.mailbox {
            Some(f) => f,
            None => return,
        };
        let edge = match facade.callback_edge(&edge_id) {
            Some(e) => e,
            None => return,
        };
        if matches!(
            edge.state,
            CallbackEdgeState::Done | CallbackEdgeState::Failed | CallbackEdgeState::TimedOut
        ) {
            return;
        }
        let _ = facade.update_callback_edge(
            &edge,
            CallbackEdgeChanges {
                state: Some(CallbackEdgeState::Done),
                child_reply_id: None,
                child_status: None,
                continuation_job_id: None,
                continuation_message_id: None,
                timeout_at: Some(None),
                diagnostics: None,
                updated_at: Some(finished_at.to_string()),
            },
        );
    }

    /// Set the parent message to RUNNING while waiting for a callback child.
    fn mark_parent_message_waiting(&self, edge: &CallbackEdgeRecord, updated_at: &str) {
        if matches!(
            edge.state,
            CallbackEdgeState::Failed | CallbackEdgeState::TimedOut | CallbackEdgeState::Done
        ) {
            return;
        }
        if let Some(facade) = &self.mailbox {
            facade.set_message_state(&edge.parent_message_id, MessageState::Running, updated_at);
        }
    }

    /// Build a terminal decision for a parent job that delegates to a callback child.
    fn delegated_terminal_decision(
        job: &JobRecord,
        edge: &CallbackEdgeRecord,
    ) -> serde_json::Value {
        let mut decision = job.terminal_decision.clone().unwrap_or_else(|| {
            serde_json::json!({
                "reply": "",
                "status": format!("{:?}", job.status).to_lowercase(),
            })
        });
        if let Some(obj) = decision.as_object_mut() {
            obj.insert("delegated".to_string(), true.into());
            obj.insert("suppress_reply".to_string(), true.into());
            obj.insert("callback_edge_id".to_string(), edge.edge_id.clone().into());
            obj.insert(
                "callback_child_job_id".to_string(),
                edge.child_job_id.clone().into(),
            );
        }
        decision
    }

    fn terminal_callback_state(state: &CallbackEdgeState) -> bool {
        matches!(
            state,
            CallbackEdgeState::ContinuationSubmitted
                | CallbackEdgeState::Done
                | CallbackEdgeState::Failed
                | CallbackEdgeState::TimedOut
        )
    }

    fn active_parent_job(&self, actor: &str) -> Option<&JobRecord> {
        let normalized = actor.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        if !self.agent_names.iter().any(|a| a == &normalized) {
            return None;
        }
        let parent_job_id = self.state.active_job(&normalized)?;
        self.get(parent_job_id)
    }

    fn message_for_job(&self, job: &JobRecord) -> Option<ccbr_mailbox::models::MessageRecord> {
        let control = self.mailbox_control.as_ref()?;
        let attempt = control.attempt_store().get_latest_by_job_id(&job.job_id)?;
        control.message_store().get_latest(&attempt.message_id)
    }

    fn validate_callback_chain(&self, parent: &JobRecord, child_agent: &str) -> Result<(), String> {
        let parent_message = self
            .message_for_job(parent)
            .ok_or_else(|| "ask --callback could not resolve parent message".to_string())?;
        let chain = self.callback_chain_for_parent(&parent_message.message_id);
        let next_depth = chain.len() as u32 + 1;
        if next_depth > self.max_callback_depth {
            return Err(format!(
                "ask --callback exceeds max callback depth {}",
                self.max_callback_depth
            ));
        }
        let mut actors: std::collections::HashSet<String> = chain
            .iter()
            .map(|e| e.parent_agent.trim().to_lowercase())
            .collect();
        for edge in &chain {
            if let Some(child) = edge.diagnostics.get("child_agent").and_then(|v| v.as_str()) {
                actors.insert(child.trim().to_lowercase());
            }
        }
        actors.insert(parent.agent_name.trim().to_lowercase());
        let target = child_agent.trim().to_lowercase();
        if actors.contains(&target) {
            return Err("ask --callback cycle detected".to_string());
        }
        Ok(())
    }

    fn callback_chain_for_parent(&self, parent_message_id: &str) -> Vec<CallbackEdgeRecord> {
        let facade = match &self.mailbox {
            Some(f) => f,
            None => return Vec::new(),
        };
        let mut chain = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut message_id = parent_message_id.to_string();
        while !message_id.is_empty() && seen.insert(message_id.clone()) {
            if let Some(edge) = facade.callback_edge_for_child_message(&message_id) {
                let next_message_id = edge.parent_message_id.clone();
                chain.push(edge);
                message_id = next_message_id;
            } else {
                break;
            }
        }
        chain
    }

    fn existing_continuation_job(&self, edge: &CallbackEdgeRecord) -> Option<&JobRecord> {
        self.job_store
            .iter()
            .rev()
            .filter(|j| j.agent_name == edge.callback_target_agent)
            .find(|j| {
                j.request
                    .route_options
                    .get("callback_edge_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s == edge.edge_id)
                    .unwrap_or(false)
            })
    }

    fn latest_child_reply(
        &self,
        edge: &CallbackEdgeRecord,
    ) -> Option<ccbr_mailbox::models::ReplyRecord> {
        let control = self.mailbox_control.as_ref()?;
        if let Some(child_reply_id) = &edge.child_reply_id {
            if let Some(reply) = control.reply_store().get_latest(child_reply_id) {
                return Some(reply);
            }
        }
        control
            .reply_store()
            .list_message(&edge.child_message_id)
            .into_iter()
            .rev()
            .find(|r| {
                !r.diagnostics
                    .get("notice")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
    }

    fn job_for_reply(&self, reply: &ccbr_mailbox::models::ReplyRecord) -> Option<JobRecord> {
        let control = self.mailbox_control.as_ref()?;
        let attempt = control.attempt_store().get_latest(&reply.attempt_id)?;
        self.get(&attempt.job_id).cloned()
    }

    fn decision_from_reply(
        &self,
        reply: &ccbr_mailbox::models::ReplyRecord,
        child_job: &JobRecord,
        fallback_finished_at: &str,
    ) -> CompletionDecision {
        let terminal = child_job.terminal_decision.clone().unwrap_or_default();
        let status = terminal
            .get("status")
            .and_then(|v| v.as_str())
            .and_then(Self::parse_completion_status)
            .unwrap_or(Self::job_status_to_completion_status(child_job.status));
        let reason = terminal
            .get("reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{:?}", child_job.status).to_lowercase());
        let mut diagnostics: serde_json::Map<String, serde_json::Value> = terminal
            .get("diagnostics")
            .cloned()
            .and_then(|v| v.as_object().cloned())
            .map(|m| m.into_iter().collect())
            .unwrap_or_default();
        if reply.reply_artifact.is_some() && !diagnostics.contains_key("reply_artifact") {
            diagnostics.insert(
                "reply_artifact".to_string(),
                reply.reply_artifact.clone().unwrap_or_default(),
            );
        }
        CompletionDecision {
            terminal: true,
            status,
            reason: Some(reason),
            confidence: Some(ccbr_completion::models::CompletionConfidence::Degraded),
            reply: reply.reply.clone(),
            anchor_seen: terminal
                .get("anchor_seen")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_started: terminal
                .get("reply_started")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_stable: terminal
                .get("reply_stable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            provider_turn_ref: terminal
                .get("provider_turn_ref")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            source_cursor: None,
            finished_at: terminal
                .get("finished_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or(Some(reply.finished_at.clone()))
                .or(Some(fallback_finished_at.to_string())),
            diagnostics,
        }
    }

    fn parse_completion_status(value: &str) -> Option<CompletionStatus> {
        match value.to_lowercase().as_str() {
            "completed" => Some(CompletionStatus::Completed),
            "cancelled" => Some(CompletionStatus::Cancelled),
            "failed" => Some(CompletionStatus::Failed),
            "incomplete" => Some(CompletionStatus::Incomplete),
            _ => None,
        }
    }

    fn job_status_to_completion_status(status: JobStatus) -> CompletionStatus {
        match status {
            JobStatus::Completed => CompletionStatus::Completed,
            JobStatus::Cancelled => CompletionStatus::Cancelled,
            JobStatus::Failed => CompletionStatus::Failed,
            _ => CompletionStatus::Incomplete,
        }
    }

    fn callback_timeout_at(&self, accepted_at: &str) -> String {
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(accepted_at) {
            let timeout = chrono::Duration::milliseconds((self.callback_timeout_s * 1000.0) as i64);
            return (ts + timeout).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        }
        accepted_at.to_string()
    }

    fn callback_edge_expired(&self, edge: &CallbackEdgeRecord) -> bool {
        let Some(timeout_at) = &edge.timeout_at else {
            return false;
        };
        let Ok(now) = chrono::DateTime::parse_from_rfc3339(&self.now()) else {
            return false;
        };
        let Ok(timeout) = chrono::DateTime::parse_from_rfc3339(timeout_at) else {
            return false;
        };
        now >= timeout
    }

    fn callback_body_summary(request: &MessageEnvelope) -> String {
        let body = Self::strip_ccbr_guidance(&request.body);
        if let Some(artifact) = &request.body_artifact {
            if let Some(obj) = artifact.as_object() {
                let path = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let bytes = obj.get("bytes").and_then(|v| v.as_str()).unwrap_or("");
                let sha256 = obj.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
                let preview = body.chars().take(800).collect::<String>();
                let mut lines = vec![
                    preview,
                    String::new(),
                    "Full original request artifact:".to_string(),
                    format!("path: {}", path),
                ];
                if !bytes.is_empty() {
                    lines.push(format!("bytes: {}", bytes));
                }
                if !sha256.is_empty() {
                    lines.push(format!("sha256: {}", sha256));
                }
                return lines.join("\n").trim_end().to_string();
            }
        }
        body
    }

    fn reply_summary(&self, decision: &CompletionDecision) -> String {
        if let Some(artifact) = decision.diagnostics.get("reply_artifact") {
            if let Some(obj) = artifact.as_object() {
                let path = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let bytes = obj.get("bytes").and_then(|v| v.as_str()).unwrap_or("");
                let sha256 = obj.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
                let reply = if decision.reply.is_empty() {
                    "(reply body stored as artifact)".to_string()
                } else {
                    decision.reply.clone()
                };
                let mut lines = vec![
                    reply,
                    String::new(),
                    "Full child reply artifact:".to_string(),
                    format!("path: {}", path),
                ];
                if !bytes.is_empty() {
                    lines.push(format!("bytes: {}", bytes));
                }
                if !sha256.is_empty() {
                    lines.push(format!("sha256: {}", sha256));
                }
                return lines.join("\n");
            }
        }
        decision.reply.clone()
    }

    fn strip_ccbr_guidance(body: &str) -> String {
        let marker = "\n\nCCBR reply guidance:";
        if let Some(idx) = body.find(marker) {
            return body[..idx].trim_end().to_string();
        }
        body.trim_end().to_string()
    }

    fn new_id(&self, prefix: &str) -> String {
        let suffix = &uuid::Uuid::new_v4().to_string().replace('-', "")[..12];
        format!("{}_{}", prefix, suffix)
    }

    fn mailbox_layout(&self) -> Option<&ccbr_storage::paths::PathLayout> {
        self.layout.as_ref()
    }

    /// Communications recovery entrypoint. Mirrors Python
    /// `lib/ccbrd/services/dispatcher_runtime/comms_recover.py::comms_recover`.
    ///
    /// **Slice 1 (noop paths).** Determines recoverability and returns `noop`
    /// for non-recoverable jobs: a RUNNING job whose agent runtime is healthy
    /// and has no recognized stale hint → `noop` / `not_recoverable`; an unknown
    /// hint is cleaned to `None` and ignored. Unknown job ids → `noop`.
    /// Recovery actions (terminal retry / stale-running cancel+retry /
    /// reply-delivery) land in later slices.
    pub fn comms_recover(&mut self, payload: &serde_json::Value) -> serde_json::Value {
        let target = match recover_target_from_payload(payload) {
            Ok(t) => t,
            Err(reason) => {
                return serde_json::json!({ "status": "noop", "noop_reason": reason });
            }
        };
        let source = match self.get(&target.job_id).cloned() {
            Some(job) => job,
            None => {
                return serde_json::json!({
                    "status": "noop",
                    "noop_reason": format!("unknown comms job: {}", target.job_id),
                });
            }
        };
        if Self::is_reply_delivery_job(&source) {
            return serde_json::json!({
                "status": "noop",
                "noop_reason": "comms recovery requires source business job",
            });
        }
        if let Some(delivery_id) = target.reply_delivery_job_id.clone() {
            return self.recover_reply_delivery(&source, &delivery_id);
        }
        let recoverability =
            self.comms_recoverability_for_job(&source, target.block_reason.as_deref());
        let block_reason = recoverability.block_reason.clone();
        if !recoverability.recoverable {
            // Mirrors Python: a non-recoverable job that was already retried
            // reports `already_retried` with the latest job id (idempotency).
            if let Some(latest) = self.already_retried_job_id(&source.job_id) {
                return serde_json::json!({
                    "job_id": source.job_id,
                    "agent_name": source.agent_name,
                    "status": "noop",
                    "block_reason": block_reason,
                    "recoverable": false,
                    "cancelled_old": null,
                    "released_event": null,
                    "retried_job": null,
                    "next_started": [],
                    "noop_reason": "already_retried",
                    "latest_job_id": latest,
                });
            }
            let noop_reason = block_reason.unwrap_or_else(|| "not_recoverable".to_string());
            return serde_json::json!({
                "job_id": source.job_id,
                "agent_name": source.agent_name,
                "status": "noop",
                "block_reason": recoverability.block_reason,
                "recoverable": false,
                "cancelled_old": null,
                "released_event": null,
                "retried_job": null,
                "next_started": [],
                "noop_reason": noop_reason,
            });
        }
        // Stale-running recovery (Slice 2): idempotency first.
        if let Some(latest) = self.already_retried_job_id(&source.job_id) {
            return serde_json::json!({
                "job_id": source.job_id,
                "agent_name": source.agent_name,
                "status": "noop",
                "block_reason": block_reason,
                "recoverable": true,
                "cancelled_old": null,
                "released_event": null,
                "retried_job": null,
                "next_started": [],
                "noop_reason": "already_retried",
                "latest_job_id": latest,
            });
        }
        if source.status == JobStatus::Running {
            // Stale-running recovery: cancel + retry + tick.  cancel() now
            // terminalizes running jobs immediately, matching Python cancel_job.
            let cancelled_receipt = self
                .cancel(&source.job_id)
                .unwrap_or_else(|_| CancelReceipt {
                    job_id: source.job_id.clone(),
                    agent_name: source.agent_name.clone(),
                    status: JobStatus::Cancelled,
                    cancelled_at: chrono::Utc::now().to_rfc3339(),
                    target_kind: source.target_kind,
                    target_name: source.target_name.clone(),
                });
            let retried = self.retry_job(&source.job_id);
            let started: Vec<&JobRecord> = self.tick();
            let retried_job = retried.as_ref().map(|j| {
                serde_json::json!({
                    "job_id": j.job_id,
                    "agent_name": j.agent_name,
                    "request": { "message_type": j.request.message_type },
                })
            });
            let next_started: Vec<serde_json::Value> = started
                .into_iter()
                .map(|j| serde_json::json!({ "job_id": j.job_id }))
                .collect();
            return serde_json::json!({
                "job_id": source.job_id,
                "agent_name": source.agent_name,
                "status": "recovered",
                "block_reason": block_reason,
                "recoverable": true,
                "cancelled_old": {
                    "job_id": cancelled_receipt.job_id,
                    "agent_name": cancelled_receipt.agent_name,
                    "status": "cancelled",
                },
                "released_event": null,
                "retried_job": retried_job,
                "next_started": next_started,
                "noop_reason": null,
            });
        }
        // Terminal-retry recovery (Slice 5): release the blocking mailbox head
        // (if any), then retry + tick. Mirrors Python `_recover_terminal_retry`.
        let released_event = self.release_blocking_head(&source);
        let retried = self.retry_job(&source.job_id);
        let started: Vec<&JobRecord> = self.tick();
        let retried_job = retried.as_ref().map(|j| {
            serde_json::json!({
                "job_id": j.job_id,
                "agent_name": j.agent_name,
                "request": { "message_type": j.request.message_type },
            })
        });
        let next_started: Vec<serde_json::Value> = started
            .into_iter()
            .map(|j| serde_json::json!({ "job_id": j.job_id }))
            .collect();
        serde_json::json!({
            "job_id": source.job_id,
            "agent_name": source.agent_name,
            "status": "recovered",
            "block_reason": block_reason,
            "recoverable": true,
            "cancelled_old": null,
            "released_event": released_event,
            "retried_job": retried_job,
            "next_started": next_started,
            "noop_reason": null,
        })
    }

    /// Mirrors Python `comms_recoverability_for_job` (Slice 1 subset: RUNNING
    /// stale detection only). Failed-terminal + reply-delivery retry cases
    /// (which need lineage / `_can_retry_job`) arrive in Slice 2+.
    /// Surface per-agent comms recoverability for the project view (mirrors
    /// Python `ProjectViewService` comms `recoverable`/`recover_target`/
    /// `block_reason` fields). One entry per agent's latest job.
    pub fn comms_recoverability_view(&self) -> Vec<serde_json::Value> {
        self.agent_names
            .iter()
            .filter_map(|agent| {
                let job = self.latest_for_agent(agent)?;
                let rec = self.comms_recoverability_for_job(job, None);
                let recover_target = if rec.recoverable {
                    serde_json::json!({ "job_id": job.job_id })
                } else {
                    serde_json::Value::Null
                };
                Some(serde_json::json!({
                    "id": job.job_id,
                    "agent_name": job.agent_name,
                    "recoverable": rec.recoverable,
                    "recover_target": recover_target,
                    "block_reason": rec.block_reason,
                }))
            })
            .collect()
    }

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
        let failed_terminal = matches!(
            job.status,
            JobStatus::Failed | JobStatus::Incomplete | JobStatus::Cancelled
        );
        if failed_terminal && self.can_retry_terminal(job) {
            let reason = format!("job_{}", format!("{:?}", job.status).to_lowercase());
            return CommsRecoverability {
                recoverable: true,
                block_reason: Some(reason),
            };
        }
        CommsRecoverability {
            recoverable: false,
            block_reason: None,
        }
    }

    /// Mirrors Python `_can_retry_job` (relaxed for the simplified dispatcher):
    /// a failed-terminal job is retryable when it has attempt lineage and its
    /// attempt is the latest for its message (no newer retry already exists).
    fn can_retry_terminal(&self, job: &JobRecord) -> bool {
        let Some(store) = self.lineage_store() else {
            return false;
        };
        let Some(attempt) = store.get_latest_by_job_id(&job.job_id) else {
            return false;
        };
        let latest = store
            .list_message(&attempt.message_id)
            .into_iter()
            .rfind(|a| a.agent_name == attempt.agent_name);
        matches!(latest, Some(l) if l.attempt_id == attempt.attempt_id)
    }

    /// Release the blocking mailbox head for `source`'s agent, if the source's
    /// inbound event is currently the pending head. Mirrors Python
    /// `_release_lineage_head_if_blocking`. Returns the released-event record.
    fn release_blocking_head(&self, source: &JobRecord) -> Option<serde_json::Value> {
        let control = self.mailbox_control.as_ref()?;
        let attempt = control
            .attempt_store()
            .get_latest_by_job_id(&source.job_id)?;
        let inbound = control
            .inbound_store()
            .get_latest_for_attempt(&attempt.agent_name, &attempt.attempt_id)?;
        let head = control
            .mailbox_kernel()
            .head_pending_event(&inbound.agent_name)?;
        if head.inbound_event_id != inbound.inbound_event_id {
            return None;
        }
        let now = chrono::Utc::now().to_rfc3339();
        let released = control.mailbox_kernel().abandon(
            &inbound.agent_name,
            &inbound.inbound_event_id,
            Some(&now),
        )?;
        Some(serde_json::json!({
            "agent_name": released.agent_name,
            "inbound_event_id": released.inbound_event_id,
            "attempt_id": released.attempt_id,
            "status": format!("{:?}", released.status).to_lowercase(),
        }))
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

    /// Retry a job: create a new job for the same agent/request with the next
    /// retry index in the attempt lineage. Mirrors Python `dispatcher.retry`.
    /// Public wrapper for the RPC handler.
    pub fn retry(&mut self, target: &str) -> serde_json::Value {
        match self.retry_job(target) {
            Some(j) => serde_json::json!({
                "target": target,
                "status": "retried",
                "job_id": j.job_id,
                "agent_name": j.agent_name,
            }),
            None => serde_json::json!({
                "target": target,
                "status": "noop",
                "noop_reason": "unknown_job",
            }),
        }
    }

    /// Create a retry job for `job_id` linked into the same attempt lineage
    /// (shared message_id, retry_index = max+1). Mirrors Python retry internals.
    fn retry_job(&mut self, job_id: &str) -> Option<JobRecord> {
        let original = self.get(job_id)?.clone();
        let message_id = self.attempt_message_id(job_id).unwrap_or_else(|| {
            format!(
                "msg_{}",
                &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
            )
        });
        let next_index = self.next_retry_index(job_id, &original.agent_name);
        let now = chrono::Utc::now().to_rfc3339();
        let status = self.initial_status(&original.agent_name);
        let new_job = JobRecord {
            job_id: format!(
                "job_{}",
                &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
            ),
            submission_id: None,
            agent_name: original.agent_name.clone(),
            provider: original.provider.clone(),
            request: original.request.clone(),
            status,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: now.clone(),
            updated_at: now,
            workspace_path: original.workspace_path.clone(),
            target_kind: original.target_kind,
            target_name: original.target_name.clone(),
        };
        self.job_store.push(new_job.clone());
        self.persist_job_to_mailbox(&new_job);
        self.record_attempt(&new_job, &message_id, next_index, AttemptState::Running);
        self.state.rebuild(&self.job_store);
        Some(new_job)
    }

    /// Message_id linking the attempt lineage for `job_id`, if an attempt store
    /// is wired and an attempt exists for the job.
    fn attempt_message_id(&self, job_id: &str) -> Option<String> {
        self.lineage_store()?
            .get_latest_by_job_id(job_id)
            .map(|a| a.message_id)
    }

    /// Next retry index = max existing retry_index for the message+agent + 1.
    fn next_retry_index(&self, job_id: &str, agent_name: &str) -> u32 {
        let Some(store) = self.lineage_store() else {
            return 1;
        };
        let Some(attempt) = store.get_latest_by_job_id(job_id) else {
            return 1;
        };
        store
            .list_message(&attempt.message_id)
            .into_iter()
            .filter(|a| a.agent_name == agent_name)
            .map(|a| a.retry_index)
            .max()
            .map(|m| m + 1)
            .unwrap_or(1)
    }

    /// If a newer retry attempt exists in the lineage for `job_id`, return its
    /// job_id (idempotency check mirroring Python `_already_retried_job_id`).
    fn already_retried_job_id(&self, job_id: &str) -> Option<String> {
        let store = self.lineage_store()?;
        let attempt = store.get_latest_by_job_id(job_id)?;
        let latest = store
            .list_message(&attempt.message_id)
            .into_iter()
            .rfind(|a| a.agent_name == attempt.agent_name);
        match latest {
            Some(l) if l.attempt_id != attempt.attempt_id => Some(l.job_id),
            _ => None,
        }
    }

    /// Mark a job terminal with a reply. Mirrors Python `dispatcher.complete`.
    /// When a source (non-reply-delivery) job COMPLETED with a reply, a
    /// reply_delivery job is auto-created for the requester (mirrors
    /// `prepare_reply_deliveries`).
    pub fn complete(&mut self, job_id: &str, status: JobStatus, reply: &str) {
        let decision = CompletionDecision {
            terminal: true,
            status: Self::job_status_to_completion_status(status),
            reason: Some(format!("{:?}", status).to_lowercase()),
            confidence: Some(ccbr_completion::models::CompletionConfidence::Degraded),
            reply: reply.to_string(),
            anchor_seen: false,
            reply_started: false,
            reply_stable: false,
            provider_turn_ref: None,
            source_cursor: None,
            finished_at: Some(self.now()),
            diagnostics: Default::default(),
        };
        self.complete_with_decision(job_id, &decision);
    }

    pub fn complete_with_decision(&mut self, job_id: &str, decision: &CompletionDecision) {
        let now = decision.finished_at.clone().unwrap_or_else(|| self.now());
        let mut delivery_request = None;
        let mut completed_job: Option<JobRecord> = None;
        if let Some(job) = self.job_store.iter_mut().find(|j| j.job_id == job_id) {
            let status = Self::completion_status_to_job_status(decision.status);
            if !job.status.is_terminal() {
                job.status = status;
                job.updated_at = now.clone();
            }
            job.terminal_decision = Some(serde_json::json!({
                "reply": decision.reply,
                "status": format!("{:?}", decision.status).to_lowercase(),
                "reason": decision.reason,
                "confidence": decision.confidence.as_ref().map(|c| format!("{:?}", c).to_lowercase()),
                "anchor_seen": decision.anchor_seen,
                "reply_started": decision.reply_started,
                "reply_stable": decision.reply_stable,
                "provider_turn_ref": decision.provider_turn_ref,
                "finished_at": now,
                "diagnostics": decision.diagnostics,
            }));
            completed_job = Some(job.clone());
            if job.request.message_type != "reply_delivery"
                && status == JobStatus::Completed
                && !decision.reply.is_empty()
            {
                delivery_request = Some((
                    job.agent_name.clone(),
                    job.request.from_actor.clone(),
                    job.request.project_id.clone(),
                    job.provider.clone(),
                ));
            }
        }
        self.state.rebuild(&self.job_store);

        if let Some(job) = completed_job {
            self.record_terminal_completion(&job, decision, &now);
            self.handle_callback_completion(&job, decision, &now);
        }

        if let Some((agent, requester, project_id, provider)) = delivery_request {
            // Don't self-deliver (requester == agent).
            if requester != agent {
                let delivery_id = self.create_reply_delivery_job(
                    job_id,
                    &agent,
                    &requester,
                    &project_id,
                    &provider,
                    &decision.reply,
                    &now,
                );
                self.reply_deliveries
                    .entry(job_id.to_string())
                    .or_default()
                    .push(delivery_id);
            }
        }
    }

    fn record_terminal_completion(
        &mut self,
        job: &JobRecord,
        decision: &CompletionDecision,
        finished_at: &str,
    ) {
        // Only integrate with the message bureau for callback-related jobs.
        // Normal job completion is handled by the execution/finalization layer;
        // recording terminal attempts here would consume mailbox heads and break
        // comms_recover tests that rely on pending inbounds.
        let is_callback_parent = self.delegated_parent_edge(job).is_some();
        let is_callback_child = self.callback_child_edge(job).is_some();
        let is_callback_continuation = job.request.message_type == "callback_continuation";
        if !is_callback_parent && !is_callback_child && !is_callback_continuation {
            return;
        }
        if let Some(facade) = &self.mailbox {
            let mb_job = crate::adapters::mailbox::to_mailbox_job_record(job);
            let mb_decision = Self::to_mailbox_completion_decision(decision, job.status);
            facade.record_attempt_terminal(&mb_job, &mb_decision, finished_at);
            if is_callback_parent {
                // Callback parents are delegated: no own reply; continuation
                // will deliver the final result.
                return;
            }
            if is_callback_child {
                // Callback child: record the reply against the child message so
                // the continuation can reference it, but do not deliver to the
                // original caller yet. Large replies are spilled to artifacts
                // so the continuation body stays under 4 KiB.
                let spilled = self.spill_decision_reply_if_needed(decision, job);
                let mb_decision = Self::to_mailbox_completion_decision(&spilled, job.status);
                let reply_id = facade.record_reply(&mb_job, &mb_decision, finished_at, false);
                if let Some(reply_id) = reply_id {
                    if let Some(child_edge) = self.callback_child_edge(job) {
                        let _ = self.submit_callback_continuation(
                            &child_edge,
                            job,
                            Some(&reply_id),
                            &spilled,
                            finished_at,
                        );
                    }
                }
                return;
            }
            // Callback continuation: deliver final reply to original caller.
            if job.status == JobStatus::Completed {
                facade.record_reply(&mb_job, &mb_decision, finished_at, true);
            }
        }
    }

    fn to_mailbox_completion_decision(
        decision: &CompletionDecision,
        job_status: JobStatus,
    ) -> MailboxCompletionDecision {
        let status = Self::daemon_job_status_to_mailbox(job_status);
        MailboxCompletionDecision {
            terminal: decision.terminal,
            status,
            reason: decision.reason.clone(),
            reply: decision.reply.clone(),
            provider_turn_ref: decision.provider_turn_ref.clone(),
            diagnostics: serde_json::to_value(&decision.diagnostics).unwrap_or_default(),
        }
    }

    fn daemon_job_status_to_mailbox(status: JobStatus) -> ccbr_mailbox::models::JobStatus {
        let text = format!("{:?}", status).to_lowercase();
        serde_json::from_value(serde_json::Value::String(text))
            .unwrap_or(ccbr_mailbox::models::JobStatus::Incomplete)
    }

    fn handle_callback_completion(
        &mut self,
        job: &JobRecord,
        _decision: &CompletionDecision,
        finished_at: &str,
    ) {
        // 1. Parent job completed with a callback child active -> mark delegated terminal.
        if let Some(edge) = self.delegated_parent_edge(job) {
            if !job.status.is_terminal() {
                return;
            }
            let terminal_decision = Self::delegated_terminal_decision(job, &edge);
            if let Some(j) = self.job_store.iter_mut().find(|j| j.job_id == job.job_id) {
                j.terminal_decision = Some(terminal_decision);
            }
            self.mark_parent_message_waiting(&edge, finished_at);
            return;
        }

        // 2. Callback continuation completed -> mark edge DONE and propagate reply to parent.
        if job.request.message_type == "callback_continuation" {
            self.mark_callback_done(job, finished_at);
        }
    }

    fn spill_decision_reply_if_needed(
        &self,
        decision: &CompletionDecision,
        job: &JobRecord,
    ) -> CompletionDecision {
        let Some(layout) = self.layout.as_ref() else {
            return decision.clone();
        };
        let force_artifact = job
            .request
            .route_options
            .get("artifact_reply")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let threshold = if force_artifact {
            Some(1)
        } else {
            Some(TEXT_ARTIFACT_SPILL_BYTES)
        };
        let prefix = format!(
            "CCBR callback reply for job {} is larger than 4 KiB and was stored as an artifact.",
            job.job_id
        );
        match maybe_spill_text(
            layout,
            &decision.reply,
            "callback-reply",
            &job.job_id,
            &prefix,
            threshold,
            None,
            Some(&self.now()),
        ) {
            Ok((reply_text, Some(artifact))) => {
                let mut next = decision.clone();
                next.reply = reply_text;
                next.diagnostics.insert(
                    "reply_artifact".to_string(),
                    serde_json::to_value(artifact.to_record()).unwrap_or_default(),
                );
                next
            }
            _ => decision.clone(),
        }
    }

    fn completion_status_to_job_status(status: CompletionStatus) -> JobStatus {
        match status {
            CompletionStatus::Completed => JobStatus::Completed,
            CompletionStatus::Cancelled => JobStatus::Cancelled,
            CompletionStatus::Failed => JobStatus::Failed,
            CompletionStatus::Incomplete => JobStatus::Incomplete,
        }
    }

    /// Create a reply_delivery job carrying `reply` from `agent` to `requester`,
    /// linked to `source_job_id`. Returns the new job id.
    #[allow(clippy::too_many_arguments)]
    fn create_reply_delivery_job(
        &mut self,
        source_job_id: &str,
        agent: &str,
        requester: &str,
        project_id: &str,
        provider: &str,
        reply: &str,
        now: &str,
    ) -> String {
        let envelope = MessageEnvelope {
            project_id: project_id.to_string(),
            to_agent: requester.to_string(),
            from_actor: agent.to_string(),
            body: reply.to_string(),
            task_id: None,
            reply_to: Some(source_job_id.to_string()),
            message_type: "reply_delivery".to_string(),
            delivery_scope: DeliveryScope::Single,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        };
        let status = self.initial_status(requester);
        let job_id = format!(
            "job_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        let job = JobRecord {
            job_id: job_id.clone(),
            submission_id: Some(source_job_id.to_string()),
            agent_name: requester.to_string(),
            provider: provider.to_string(),
            request: envelope,
            status,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            workspace_path: None,
            target_kind: TargetKind::Agent,
            target_name: requester.to_string(),
        };
        self.job_store.push(job.clone());
        self.persist_job_to_mailbox(&job);
        let message_id = format!(
            "msg_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        self.record_attempt(&job, &message_id, 0, AttemptState::Running);
        self.state.rebuild(&self.job_store);
        job_id
    }

    /// Mirrors `is_reply_delivery_job`.
    fn is_reply_delivery_job(job: &JobRecord) -> bool {
        job.request.message_type == "reply_delivery"
    }

    /// Recover a failed reply_delivery: create a new delivery job for the
    /// source's reply (mirrors Python `_recover_reply_delivery`). Idempotent via
    /// the reply-delivery lineage.
    fn recover_reply_delivery(
        &mut self,
        source: &JobRecord,
        delivery_id: &str,
    ) -> serde_json::Value {
        let delivery_status = self.get(delivery_id).map(|j| j.status);
        let failed = matches!(
            delivery_status,
            Some(JobStatus::Failed) | Some(JobStatus::Incomplete) | Some(JobStatus::Cancelled)
        );
        if !failed {
            return serde_json::json!({
                "job_id": source.job_id,
                "agent_name": source.agent_name,
                "status": "noop",
                "block_reason": null,
                "recoverable": false,
                "cancelled_old": null,
                "released_event": null,
                "retried_job": null,
                "next_started": [],
                "noop_reason": "not_recoverable",
            });
        }
        let deliveries = self
            .reply_deliveries
            .get(&source.job_id)
            .cloned()
            .unwrap_or_default();
        // Idempotency: a recovery delivery already exists for this source.
        if deliveries.len() > 1 {
            let newest = deliveries.iter().rfind(|id| *id != delivery_id).cloned();
            return serde_json::json!({
                "job_id": source.job_id,
                "agent_name": source.agent_name,
                "status": "noop",
                "block_reason": "reply_delivery_failed",
                "recoverable": true,
                "cancelled_old": null,
                "released_event": null,
                "retried_job": null,
                "next_started": [],
                "noop_reason": "already_retried",
                "latest_job_id": newest,
            });
        }
        let now = chrono::Utc::now().to_rfc3339();
        let reply = source
            .terminal_decision
            .as_ref()
            .and_then(|d| d.get("reply"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let new_id = self.create_reply_delivery_job(
            &source.job_id,
            &source.agent_name,
            &source.request.from_actor,
            &source.request.project_id,
            &source.provider,
            &reply,
            &now,
        );
        self.reply_deliveries
            .entry(source.job_id.clone())
            .or_default()
            .push(new_id.clone());
        let started: Vec<&JobRecord> = self.tick();
        let next_started: Vec<serde_json::Value> = started
            .into_iter()
            .map(|j| serde_json::json!({ "job_id": j.job_id }))
            .collect();
        serde_json::json!({
            "job_id": source.job_id,
            "agent_name": source.agent_name,
            "status": "recovered",
            "block_reason": "reply_delivery_failed",
            "recoverable": true,
            "cancelled_old": null,
            "released_event": null,
            "retried_job": {
                "job_id": new_id,
                "request": { "message_type": "reply_delivery" },
            },
            "next_started": next_started,
            "noop_reason": null,
            "recoverability_after": { "recoverable": false },
        })
    }

    /// Promote one queued job per agent to running when no active job exists.
    pub fn tick(&mut self) -> Vec<&JobRecord> {
        self.sweep_callback_timeouts();
        self.repair_callback_edges();
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
    fn cancel_unknown_job_is_idempotent() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.cancel("no-such-job").unwrap();
        assert_eq!(receipt.job_id, "no-such-job");
        assert_eq!(receipt.status, JobStatus::Cancelled);
    }

    #[test]
    fn cancel_queued_job_marks_it_cancelled() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();

        let cancel_receipt = dispatcher.cancel(&job_id).unwrap();
        assert_eq!(cancel_receipt.status, JobStatus::Cancelled);

        let job = dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.cancel_requested_at.is_some());
        assert_eq!(dispatcher.state.queue_depth("claude"), 0);
        assert!(dispatcher.state.active_job("claude").is_none());
    }

    #[test]
    fn cancel_running_job_terminalizes_immediately() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();
        dispatcher.tick();

        let job_before = dispatcher.get(&job_id).unwrap();
        assert_eq!(job_before.status, JobStatus::Running);
        assert!(job_before.cancel_requested_at.is_none());

        let cancel_receipt = dispatcher.cancel(&job_id).unwrap();
        assert_eq!(cancel_receipt.status, JobStatus::Cancelled);

        let job = dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.cancel_requested_at.is_some());
        assert!(job.terminal_decision.is_some());
        assert!(dispatcher.state.active_job("claude").is_none());
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
    fn cancel_completed_job_errors() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude", "hello"), "claude", None);
        let job_id = receipt.jobs[0].job_id.clone();
        dispatcher.tick();
        dispatcher.update_job_status(&job_id, JobStatus::Completed, None);

        let result = dispatcher.cancel(&job_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already terminal"));
        assert_eq!(
            dispatcher.get(&job_id).unwrap().status,
            JobStatus::Completed
        );
    }
}
