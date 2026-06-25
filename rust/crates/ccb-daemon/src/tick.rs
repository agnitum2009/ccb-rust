//! Mirrors Python `lib/ccbd/services/job_heartbeat_runtime/tick.py`.
//! 1:1 file alignment stub.

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Main entry point for ticking job heartbeat
pub fn tick_job_heartbeat(
    service: &dyn HeartbeatService,
    dispatcher: &mut dyn Dispatcher,
    job: &Job,
) -> Result<bool> {
    let context = build_heartbeat_tick_context(service, dispatcher, job)?;
    let context = match context {
        Some(ctx) => ctx,
        None => return Ok(false),
    };

    match context.decision.action {
        HeartbeatAction::Reset => handle_reset_heartbeat(service, dispatcher, job, &context),
        _ if !context.decision.notice_due => Ok(true),
        _ if heartbeat_timeout_due(service, &context)? => {
            terminalize_heartbeat_timeout(service, dispatcher, job, &context)
        }
        _ => record_internal_heartbeat(service, dispatcher, job, &context),
    }
}

/// Build heartbeat tick context from job state
pub fn build_heartbeat_tick_context(
    service: &dyn HeartbeatService,
    dispatcher: &mut dyn Dispatcher,
    job: &Job,
) -> Result<Option<HeartbeatTickContext>> {
    let snapshot = dispatcher.get_snapshot(&job.job_id)?;
    if snapshot_is_terminal(&snapshot) {
        service.remove(&service.subject_kind(), &job.job_id)?;
        return Ok(None);
    }

    let observed_last_progress_at = snapshot
        .as_ref()
        .and_then(|s| s.updated_at.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| job.updated_at.clone())
        .filter(|s| !s.is_empty());

    let observed_last_progress_at = match observed_last_progress_at {
        Some(ts) => ts,
        None => return Ok(None),
    };

    let prior_state = service.load(&service.subject_kind(), &job.job_id)?;
    let now = service.now();

    let (next_state, decision) = evaluate_heartbeat(
        service.policy(),
        &service.subject_kind(),
        &job.job_id,
        &job.agent_name,
        &observed_last_progress_at,
        &now,
        prior_state.as_ref(),
    )?;

    Ok(Some(HeartbeatTickContext {
        snapshot,
        observed_last_progress_at,
        now,
        next_state,
        decision,
    }))
}

/// Handle heartbeat reset action
pub fn handle_reset_heartbeat(
    service: &dyn HeartbeatService,
    dispatcher: &mut dyn Dispatcher,
    job: &Job,
    context: &HeartbeatTickContext,
) -> Result<bool> {
    service.save(context.next_state.clone())?;
    dispatcher.append_event(
        job,
        "job_heartbeat_reset",
        serde_json::json!({
            "subject_kind": service.subject_kind(),
            "action": match context.decision.action {
                HeartbeatAction::Reset => "reset",
                HeartbeatAction::Observe => "observe",
                HeartbeatAction::Timeout => "timeout",
            },
            "notice_count": context.decision.notice_count,
            "last_progress_at": context.decision.last_progress_at,
        }),
        &context.now,
    )?;
    Ok(true)
}

/// Check if heartbeat timeout is due
pub fn heartbeat_timeout_due(
    service: &dyn HeartbeatService,
    context: &HeartbeatTickContext,
) -> Result<bool> {
    let limit = service.terminal_notice_count();
    Ok(limit.is_some() && context.decision.notice_count >= limit.unwrap_or(0))
}

/// Record internal heartbeat observation
pub fn record_internal_heartbeat(
    service: &dyn HeartbeatService,
    dispatcher: &mut dyn Dispatcher,
    job: &Job,
    context: &HeartbeatTickContext,
) -> Result<bool> {
    let mailbox_target =
        normalize_mailbox_target(&job.request.from_actor, &dispatcher.known_mailbox_targets())?;

    let diagnostics = heartbeat_diagnostics(
        job,
        &context.decision,
        context.snapshot.as_ref(),
        &mailbox_target,
        &service.subject_kind(),
    )?;

    service.save(context.next_state.clone())?;
    dispatcher.append_event(job, "job_heartbeat_observed", diagnostics, &context.now)?;
    Ok(true)
}

/// Terminalize heartbeat timeout (finalize job as timed out)
pub fn terminalize_heartbeat_timeout(
    service: &dyn HeartbeatService,
    dispatcher: &mut dyn Dispatcher,
    job: &Job,
    context: &HeartbeatTickContext,
) -> Result<bool> {
    let mailbox_target =
        normalize_mailbox_target(&job.request.from_actor, &dispatcher.known_mailbox_targets())?;

    let diagnostics = heartbeat_diagnostics(
        job,
        &context.decision,
        context.snapshot.as_ref(),
        &mailbox_target,
        &service.subject_kind(),
    )?;

    service.save(context.next_state.clone())?;
    dispatcher.append_event(job, "job_heartbeat_timeout", diagnostics, &context.now)?;

    let timeout_decision = heartbeat_timeout_decision(
        job,
        &context.decision,
        context.snapshot.as_ref(),
        &context.now,
    )?;

    dispatcher.complete(&job.job_id, timeout_decision)?;
    service.remove(&service.subject_kind(), &job.job_id)?;
    Ok(false)
}

// Helper functions and types (simplified for stub)

pub trait HeartbeatService {
    fn subject_kind(&self) -> String;
    fn policy(&self) -> &HeartbeatPolicy;
    fn now(&self) -> String;
    fn load(&self, kind: &str, id: &str) -> Result<Option<HeartbeatState>>;
    fn save(&self, state: HeartbeatState) -> Result<()>;
    fn remove(&self, kind: &str, id: &str) -> Result<()>;
    fn terminal_notice_count(&self) -> Option<u32>;
}

pub trait Dispatcher {
    fn get_snapshot(&self, job_id: &str) -> Result<Option<JobSnapshot>>;
    fn known_mailbox_targets(&self) -> Vec<String>;
    fn append_event(
        &self,
        job: &Job,
        event_type: &str,
        payload: serde_json::Value,
        timestamp: &str,
    ) -> Result<()>;
    fn complete(&mut self, job_id: &str, decision: serde_json::Value) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Job {
    pub job_id: String,
    pub agent_name: String,
    pub updated_at: Option<String>,
    pub request: JobRequest,
}

#[derive(Debug, Clone)]
pub struct JobRequest {
    pub from_actor: String,
}

#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HeartbeatState {
    pub subject_kind: String,
    pub subject_id: String,
    pub owner: String,
    pub last_progress_at: String,
    pub last_notice_at: Option<String>,
    pub heartbeat_started_at: Option<String>,
    pub notice_count: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct HeartbeatPolicy {
    pub timeout_seconds: u64,
    pub repeat_interval_seconds: u64,
    pub max_notices: Option<u32>,
}

pub fn evaluate_heartbeat(
    policy: &HeartbeatPolicy,
    subject_kind: &str,
    subject_id: &str,
    owner: &str,
    observed_last_progress_at: &str,
    now: &str,
    state: Option<&HeartbeatState>,
) -> Result<(HeartbeatState, HeartbeatDecision)> {
    let engine_policy = ccb_heartbeat::models::HeartbeatPolicy::new(
        policy.timeout_seconds as f64,
        policy.repeat_interval_seconds as f64,
        policy.max_notices,
    )
    .map_err(|e| format!("invalid heartbeat policy: {e}"))?;

    let engine_state = state
        .as_ref()
        .map(|s| ccb_heartbeat::models::HeartbeatState {
            subject_kind: s.subject_kind.clone(),
            subject_id: s.subject_id.clone(),
            owner: s.owner.clone(),
            last_progress_at: s.last_progress_at.clone(),
            last_notice_at: s.last_notice_at.clone(),
            heartbeat_started_at: s.heartbeat_started_at.clone(),
            notice_count: s.notice_count,
            updated_at: s.updated_at.clone(),
        });

    let (next_engine_state, engine_decision) = ccb_heartbeat::engine::evaluate_heartbeat(
        &engine_policy,
        subject_kind,
        subject_id,
        owner,
        observed_last_progress_at,
        now,
        engine_state.as_ref(),
    );

    let next_state = HeartbeatState {
        subject_kind: next_engine_state.subject_kind,
        subject_id: next_engine_state.subject_id,
        owner: next_engine_state.owner,
        last_progress_at: next_engine_state.last_progress_at,
        last_notice_at: next_engine_state.last_notice_at,
        heartbeat_started_at: next_engine_state.heartbeat_started_at,
        notice_count: next_engine_state.notice_count,
        updated_at: next_engine_state.updated_at,
    };

    let limit_reached = policy
        .max_notices
        .map(|limit| engine_decision.notice_count >= limit)
        .unwrap_or(false);

    let action = match engine_decision.action {
        ccb_heartbeat::models::HeartbeatAction::Reset => HeartbeatAction::Reset,
        ccb_heartbeat::models::HeartbeatAction::Idle if limit_reached => HeartbeatAction::Timeout,
        ccb_heartbeat::models::HeartbeatAction::Idle => HeartbeatAction::Observe,
        ccb_heartbeat::models::HeartbeatAction::Enter => HeartbeatAction::Observe,
        ccb_heartbeat::models::HeartbeatAction::Repeat => HeartbeatAction::Observe,
    };

    let decision = HeartbeatDecision {
        action,
        notice_due: engine_decision.action != ccb_heartbeat::models::HeartbeatAction::Idle
            || limit_reached,
        notice_count: engine_decision.notice_count,
        last_progress_at: engine_decision.last_progress_at,
    };

    Ok((next_state, decision))
}

pub fn normalize_mailbox_target(from: &str, _known: &[String]) -> Result<String> {
    Ok(from.to_string())
}

pub fn heartbeat_diagnostics(
    job: &Job,
    _decision: &HeartbeatDecision,
    _snapshot: Option<&JobSnapshot>,
    mailbox_target: &str,
    subject_kind: &str,
) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "job_id": job.job_id,
        "agent_name": job.agent_name,
        "mailbox_target": mailbox_target,
        "subject_kind": subject_kind,
    }))
}

pub fn heartbeat_timeout_decision(
    _job: &Job,
    _decision: &HeartbeatDecision,
    _snapshot: Option<&JobSnapshot>,
    finished_at: &str,
) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "status": "timeout",
        "finished_at": finished_at,
    }))
}

fn snapshot_is_terminal(_snapshot: &Option<JobSnapshot>) -> bool {
    // Stub: assume non-terminal for now
    false
}

// Local type definitions for stub implementation

#[derive(Debug, Clone)]
pub struct HeartbeatTickContext {
    pub snapshot: Option<JobSnapshot>,
    pub observed_last_progress_at: String,
    pub now: String,
    pub next_state: HeartbeatState,
    pub decision: HeartbeatDecision,
}

#[derive(Debug, Clone)]
pub struct HeartbeatDecision {
    pub action: HeartbeatAction,
    pub notice_due: bool,
    pub notice_count: u32,
    pub last_progress_at: String,
}

impl HeartbeatDecision {
    pub fn as_str(&self) -> &'static str {
        match self.action {
            HeartbeatAction::Reset => "reset",
            HeartbeatAction::Observe => "observe",
            HeartbeatAction::Timeout => "timeout",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatAction {
    Reset,
    Observe,
    Timeout,
}
