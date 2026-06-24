//! Mirrors Python `lib/ccbrd/services/dispatcher_runtime/submission_service.py`.
//!
//! This is a 1:1 alignment stub.  It provides the public planning helpers used
//! by the `/ask` submission path and keeps all heavy dispatcher dependencies
//! behind a small trait so that the pure logic can be unit-tested with mocks.

use ccbr_jobs::models::{DeliveryScope, JobRecord, MessageEnvelope, TargetKind};
use ccbr_mailbox::models::{AttemptRecord, AttemptState, MessageRecord};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors and simple domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
#[error("{message}")]
pub struct DispatchError {
    message: String,
}

impl DispatchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Minimal spec for an agent, matching the Python registry shape.
#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub provider: String,
}

/// Minimal runtime snapshot for an agent.
#[derive(Debug, Clone, Default)]
pub struct AgentRuntime {
    pub project_id: Option<String>,
    pub workspace_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Submission model types
// ---------------------------------------------------------------------------

/// A single job draft inside a submission plan.
///
/// Mirrors Python `_JobDraft`.
#[derive(Debug, Clone)]
pub struct SubmissionItem {
    pub agent_name: String,
    pub provider: String,
    pub request: MessageEnvelope,
    pub target_kind: TargetKind,
    pub target_name: String,
    pub provider_instance: Option<String>,
    pub provider_options: Option<Value>,
    pub workspace_path: Option<String>,
}

/// Alias so callers can use the Python-ish name.
pub type JobDraft = SubmissionItem;

/// The result of planning a submission.
///
/// Mirrors Python `_SubmissionPlan`.
#[derive(Debug, Clone)]
pub struct SubmissionPlan {
    pub project_id: String,
    pub from_actor: String,
    pub request: MessageEnvelope,
    pub task_id: Option<String>,
    pub drafts: Vec<SubmissionItem>,
    pub submission_id: Option<String>,
    pub target_scope: Option<String>,
    pub origin_message_id: Option<String>,
}

/// Convenience alias requested by the task brief.
pub type SubmissionPlanResult = Result<SubmissionPlan, DispatchError>;

// ---------------------------------------------------------------------------
// Dispatcher capability trait
// ---------------------------------------------------------------------------

/// Minimal view of the dispatcher needed to plan submissions.
pub trait Dispatcher {
    fn validate_sender(&self, from_actor: &str) -> Result<(), DispatchError>;
    fn resolve_targets(&self, request: &MessageEnvelope) -> Vec<String>;
    fn validate_targets_available(&self, targets: &[String]) -> Result<(), DispatchError>;
    fn new_id(&self, prefix: &str) -> String;
    fn dispatch_error(&self, message: &str) -> DispatchError;
    fn agent_spec(&self, agent_name: &str) -> Option<AgentSpec>;
    fn agent_runtime(&self, agent_name: &str) -> Option<AgentRuntime>;
    fn get_message(&self, message_id: &str) -> Option<MessageRecord>;
    fn attempt_latest(&self, target: &str) -> Option<AttemptRecord>;
    fn attempt_latest_by_job_id(&self, job_id: &str) -> Option<AttemptRecord>;
    fn attempts_for_message(&self, message_id: &str) -> Vec<AttemptRecord>;
    fn get_job(&self, agent_name: &str, job_id: &str) -> Option<JobRecord>;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const BODY_ARTIFACT_SPILL_BYTES: usize = 4096;

const TERMINAL_ATTEMPT_STATES: &[AttemptState] = &[
    AttemptState::Completed,
    AttemptState::Incomplete,
    AttemptState::Failed,
    AttemptState::Cancelled,
    AttemptState::Superseded,
    AttemptState::DeadLetter,
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Plan a fresh `/ask` submission.
///
/// Mirrors Python `_plan_agent_submission`.  This is also exposed as
/// `plan_submission` for callers that expect a single entry point.
pub fn plan_submission<D: Dispatcher>(
    dispatcher: &D,
    request: &MessageEnvelope,
) -> SubmissionPlanResult {
    plan_agent_submission(dispatcher, request)
}

pub fn plan_agent_submission<D: Dispatcher>(
    dispatcher: &D,
    request: &MessageEnvelope,
) -> SubmissionPlanResult {
    dispatcher.validate_sender(&request.from_actor)?;
    validate_request_body_artifact(dispatcher, request)?;
    validate_callback_request(dispatcher, request)?;

    let targets = dispatcher.resolve_targets(request);
    if targets.is_empty() {
        return Err(
            dispatcher.dispatch_error("no eligible target agents are alive for this request")
        );
    }
    dispatcher.validate_targets_available(&targets)?;

    let submission_id = if request.delivery_scope == DeliveryScope::Broadcast {
        Some(dispatcher.new_id("sub"))
    } else {
        None
    };

    let drafts = drafts_for_agents(dispatcher, request, &targets);

    Ok(SubmissionPlan {
        project_id: request.project_id.clone(),
        from_actor: request.from_actor.clone(),
        request: request.clone(),
        task_id: request.task_id.clone(),
        drafts,
        submission_id: submission_id.clone(),
        target_scope: submission_id.as_ref().map(|_| "all".to_string()),
        origin_message_id: None,
    })
}

/// Plan a resubmission of an existing message.
///
/// Mirrors Python `_plan_message_resubmission`.
pub fn plan_message_resubmission<D: Dispatcher>(
    dispatcher: &D,
    message_id: &str,
) -> SubmissionPlanResult {
    let original_message = dispatcher
        .get_message(message_id)
        .ok_or_else(|| dispatcher.dispatch_error(&format!("message not found: {message_id}")))?;

    let latest_attempts =
        require_attempt_lineage(dispatcher, message_id, &original_message.target_agents)?;
    ensure_no_active_attempts(dispatcher, &latest_attempts)?;

    let jobs = jobs_for_resubmission(
        dispatcher,
        &original_message.target_agents,
        &latest_attempts,
    )?;
    let request = resubmission_request(&original_message, &jobs)
        .ok_or_else(|| dispatcher.dispatch_error("no source job for resubmission request"))?;

    dispatcher.validate_sender(&request.from_actor)?;
    validate_request_body_artifact(dispatcher, &request)?;
    dispatcher.validate_targets_available(&original_message.target_agents)?;

    let submission_id = if request.delivery_scope == DeliveryScope::Broadcast {
        Some(dispatcher.new_id("sub"))
    } else {
        None
    };

    let task_id = request.task_id.clone();
    let drafts = drafts_for_agents(dispatcher, &request, &original_message.target_agents);

    Ok(SubmissionPlan {
        project_id: request.project_id.clone(),
        from_actor: request.from_actor.clone(),
        request,
        task_id,
        drafts,
        submission_id: submission_id.clone(),
        target_scope: submission_id.as_ref().map(|_| "all".to_string()),
        origin_message_id: Some(message_id.to_string()),
    })
}

/// Resolve a retry target by agent name, falling back to job id.
///
/// Mirrors Python `_resolve_retry_attempt`.
pub fn resolve_retry_attempt<D: Dispatcher>(
    dispatcher: &D,
    target: &str,
) -> Result<AttemptRecord, DispatchError> {
    if let Some(attempt) = dispatcher.attempt_latest(target) {
        return Ok(attempt);
    }
    if let Some(attempt) = dispatcher.attempt_latest_by_job_id(target) {
        return Ok(attempt);
    }
    Err(dispatcher.dispatch_error(&format!("retry target not found: {target}")))
}

/// Find the latest attempt record per target agent for a message.
///
/// Mirrors Python `_latest_attempts_by_agent`.
pub fn latest_attempts_by_agent<D: Dispatcher>(
    dispatcher: &D,
    message_id: &str,
) -> HashMap<String, AttemptRecord> {
    let mut by_attempt_id: HashMap<String, AttemptRecord> = HashMap::new();
    for record in dispatcher.attempts_for_message(message_id) {
        by_attempt_id.insert(record.attempt_id.clone(), record);
    }

    let mut by_agent: HashMap<String, AttemptRecord> = HashMap::new();
    for record in by_attempt_id.into_values() {
        let keep = by_agent
            .get(&record.agent_name)
            .is_none_or(|current| attempt_sort_key(&record) > attempt_sort_key(current));
        if keep {
            by_agent.insert(record.agent_name.clone(), record);
        }
    }
    by_agent
}

/// Ensure an agent is registered before it is targeted.
///
/// Mirrors Python `_ensure_agent_target_ready`.
pub fn ensure_agent_target_ready<D: Dispatcher>(
    dispatcher: &D,
    agent_name: &str,
) -> Result<(), DispatchError> {
    dispatcher
        .agent_spec(agent_name)
        .ok_or_else(|| dispatcher.dispatch_error(&format!("agent not registered: {agent_name}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Build a per-agent message envelope from a broadcast request.
///
/// Mirrors Python `_message_for_agent`.
pub fn message_for_agent(request: &MessageEnvelope, agent_name: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: request.project_id.clone(),
        to_agent: agent_name.to_string(),
        from_actor: request.from_actor.clone(),
        body: request.body.clone(),
        task_id: request.task_id.clone(),
        reply_to: request.reply_to.clone(),
        message_type: request.message_type.clone(),
        delivery_scope: DeliveryScope::Agent,
        silence_on_success: request.silence_on_success,
        route_options: request.route_options.clone(),
        body_artifact: request.body_artifact.clone(),
    }
}

/// Sort key for comparing attempt lineage records.
///
/// Mirrors Python `_attempt_sort_key`.
pub fn attempt_sort_key(record: &AttemptRecord) -> (u32, String, String) {
    (
        record.retry_index,
        record.updated_at.clone(),
        record.attempt_id.clone(),
    )
}

/// Detect whether a request asks for callback routing.
///
/// Mirrors Python `request_callback_route`.
pub fn request_callback_route(request: &MessageEnvelope) -> bool {
    request
        .route_options
        .as_object()
        .and_then(|opts| opts.get("mode"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().eq_ignore_ascii_case("callback"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn drafts_for_agents<D: Dispatcher>(
    dispatcher: &D,
    request: &MessageEnvelope,
    targets: &[String],
) -> Vec<SubmissionItem> {
    targets
        .iter()
        .filter_map(|agent_name| {
            let spec = dispatcher.agent_spec(agent_name)?;
            Some(SubmissionItem {
                agent_name: agent_name.clone(),
                provider: spec.provider.clone(),
                request: message_for_agent(request, agent_name),
                target_kind: TargetKind::Agent,
                target_name: agent_name.clone(),
                provider_instance: None,
                provider_options: None,
                workspace_path: None,
            })
        })
        .collect()
}

fn require_attempt_lineage<D: Dispatcher>(
    dispatcher: &D,
    message_id: &str,
    target_agents: &[String],
) -> Result<HashMap<String, AttemptRecord>, DispatchError> {
    let latest_attempts = latest_attempts_by_agent(dispatcher, message_id);
    if latest_attempts.is_empty() {
        return Err(dispatcher.dispatch_error(&format!(
            "message has no attempts to resubmit: {message_id}"
        )));
    }
    let missing: Vec<String> = target_agents
        .iter()
        .filter(|a| !latest_attempts.contains_key(*a))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(dispatcher.dispatch_error(&format!(
            "message is missing attempt lineage for agents: {}",
            missing.join(", ")
        )));
    }
    Ok(latest_attempts)
}

fn ensure_no_active_attempts<D: Dispatcher>(
    dispatcher: &D,
    latest_attempts: &HashMap<String, AttemptRecord>,
) -> Result<(), DispatchError> {
    let terminal: HashSet<AttemptState> = TERMINAL_ATTEMPT_STATES.iter().copied().collect();
    let active_agents: Vec<String> = latest_attempts
        .iter()
        .filter(|(_, attempt)| !terminal.contains(&attempt.attempt_state))
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .collect();
    if !active_agents.is_empty() {
        return Err(dispatcher.dispatch_error(&format!(
            "message still has active attempts: {}",
            active_agents.join(", ")
        )));
    }
    Ok(())
}

fn jobs_for_resubmission<D: Dispatcher>(
    dispatcher: &D,
    target_agents: &[String],
    latest_attempts: &HashMap<String, AttemptRecord>,
) -> Result<Vec<JobRecord>, DispatchError> {
    let mut jobs = Vec::new();
    for agent_name in target_agents {
        ensure_agent_target_ready(dispatcher, agent_name)?;
        let attempt = &latest_attempts[agent_name];
        let job = dispatcher
            .get_job(agent_name, &attempt.job_id)
            .ok_or_else(|| {
                dispatcher.dispatch_error(&format!(
                    "job not found for attempt: {}",
                    attempt.attempt_id
                ))
            })?;
        jobs.push(job);
    }
    Ok(jobs)
}

fn resubmission_request(
    original_message: &MessageRecord,
    jobs: &[JobRecord],
) -> Option<MessageEnvelope> {
    let source = jobs.first()?;
    let delivery_scope = if original_message.target_agents.len() > 1 {
        DeliveryScope::Broadcast
    } else {
        DeliveryScope::Agent
    };
    let to_agent = if delivery_scope == DeliveryScope::Broadcast {
        "all".to_string()
    } else {
        original_message.target_agents.first()?.clone()
    };

    Some(MessageEnvelope {
        project_id: source.request.project_id.clone(),
        to_agent,
        from_actor: original_message.from_actor.clone(),
        body: source.request.body.clone(),
        task_id: source.request.task_id.clone(),
        reply_to: source.request.reply_to.clone(),
        message_type: source.request.message_type.clone(),
        delivery_scope,
        silence_on_success: source.request.silence_on_success,
        route_options: source.request.route_options.clone(),
        body_artifact: source.request.body_artifact.clone(),
    })
}

fn validate_request_body_artifact<D: Dispatcher>(
    dispatcher: &D,
    request: &MessageEnvelope,
) -> Result<(), DispatchError> {
    if request.body_artifact.is_some() {
        if request.body.len() > BODY_ARTIFACT_SPILL_BYTES {
            return Err(dispatcher.dispatch_error(
                "ask body stub exceeds 4 KiB even though a body artifact is present",
            ));
        }
        // Full artifact validation is left to the storage layer; the stub
        // accepts any present artifact reference.
        return Ok(());
    }
    if request.body.len() > BODY_ARTIFACT_SPILL_BYTES {
        return Err(dispatcher.dispatch_error(
            "ask body exceeds 4 KiB and must be submitted with a CCBR body artifact",
        ));
    }
    Ok(())
}

fn validate_callback_request<D: Dispatcher>(
    dispatcher: &D,
    request: &MessageEnvelope,
) -> Result<(), DispatchError> {
    if !request_callback_route(request) {
        return Ok(());
    }
    // Stub: enforce the single-target invariant required by --callback.
    if request.delivery_scope != DeliveryScope::Agent {
        return Err(dispatcher.dispatch_error("ask --callback supports exactly one target agent"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_jobs::models::JobStatus;
    use std::cell::RefCell;

    fn envelope(body: &str) -> MessageEnvelope {
        MessageEnvelope {
            project_id: "proj".to_string(),
            to_agent: "claude".to_string(),
            from_actor: "user".to_string(),
            body: body.to_string(),
            task_id: Some("task-1".to_string()),
            reply_to: None,
            message_type: "ask".to_string(),
            delivery_scope: DeliveryScope::Agent,
            silence_on_success: false,
            route_options: Value::Object(Default::default()),
            body_artifact: None,
        }
    }

    struct TestDispatcher {
        specs: HashMap<String, AgentSpec>,
        messages: HashMap<String, MessageRecord>,
        attempts: RefCell<Vec<AttemptRecord>>,
        jobs: RefCell<Vec<JobRecord>>,
        targets: Vec<String>,
        next_id: RefCell<u32>,
    }

    impl TestDispatcher {
        fn new(targets: Vec<String>) -> Self {
            Self {
                specs: HashMap::new(),
                messages: HashMap::new(),
                attempts: RefCell::new(Vec::new()),
                jobs: RefCell::new(Vec::new()),
                targets,
                next_id: RefCell::new(1),
            }
        }

        fn with_spec(mut self, name: &str, provider: &str) -> Self {
            self.specs.insert(
                name.to_string(),
                AgentSpec {
                    provider: provider.to_string(),
                },
            );
            self
        }

        fn with_message(mut self, id: &str, record: MessageRecord) -> Self {
            self.messages.insert(id.to_string(), record);
            self
        }

        fn with_attempt(&self, record: AttemptRecord) {
            self.attempts.borrow_mut().push(record);
        }

        fn with_job(&self, record: JobRecord) {
            self.jobs.borrow_mut().push(record);
        }
    }

    impl Dispatcher for TestDispatcher {
        fn validate_sender(&self, _from_actor: &str) -> Result<(), DispatchError> {
            Ok(())
        }

        fn resolve_targets(&self, request: &MessageEnvelope) -> Vec<String> {
            if self.targets.is_empty() {
                vec![request.to_agent.clone()]
            } else {
                self.targets.clone()
            }
        }

        fn validate_targets_available(&self, _targets: &[String]) -> Result<(), DispatchError> {
            Ok(())
        }

        fn new_id(&self, prefix: &str) -> String {
            let n = *self.next_id.borrow();
            *self.next_id.borrow_mut() += 1;
            format!("{prefix}_{n}")
        }

        fn dispatch_error(&self, message: &str) -> DispatchError {
            DispatchError::new(message)
        }

        fn agent_spec(&self, agent_name: &str) -> Option<AgentSpec> {
            self.specs.get(agent_name).cloned()
        }

        fn agent_runtime(&self, _agent_name: &str) -> Option<AgentRuntime> {
            None
        }

        fn get_message(&self, message_id: &str) -> Option<MessageRecord> {
            self.messages.get(message_id).cloned()
        }

        fn attempt_latest(&self, target: &str) -> Option<AttemptRecord> {
            self.attempts
                .borrow()
                .iter()
                .rfind(|a| a.agent_name == target)
                .cloned()
        }

        fn attempt_latest_by_job_id(&self, job_id: &str) -> Option<AttemptRecord> {
            self.attempts
                .borrow()
                .iter()
                .rfind(|a| a.job_id == job_id)
                .cloned()
        }

        fn attempts_for_message(&self, message_id: &str) -> Vec<AttemptRecord> {
            self.attempts
                .borrow()
                .iter()
                .filter(|a| a.message_id == message_id)
                .cloned()
                .collect()
        }

        fn get_job(&self, agent_name: &str, job_id: &str) -> Option<JobRecord> {
            self.jobs
                .borrow()
                .iter()
                .find(|j| j.agent_name == agent_name && j.job_id == job_id)
                .cloned()
        }
    }

    #[test]
    fn test_message_for_agent_sets_single_target() {
        let req = envelope("hello");
        let per_agent = message_for_agent(&req, "gemini");
        assert_eq!(per_agent.to_agent, "gemini");
        assert_eq!(per_agent.delivery_scope, DeliveryScope::Agent);
        assert_eq!(per_agent.body, "hello");
    }

    #[test]
    fn test_request_callback_route_detects_mode() {
        let mut req = envelope("hello");
        assert!(!request_callback_route(&req));
        req.route_options = serde_json::json!({"mode": "callback"});
        assert!(request_callback_route(&req));
    }

    #[test]
    fn test_validate_request_body_artifact_enforces_spill_limit() {
        let dispatcher = TestDispatcher::new(vec!["claude".to_string()]);
        let big_body = "x".repeat(BODY_ARTIFACT_SPILL_BYTES + 1);
        let req = envelope(&big_body);
        let err = validate_request_body_artifact(&dispatcher, &req)
            .expect_err("should fail without artifact");
        assert!(err
            .to_string()
            .contains("must be submitted with a CCBR body artifact"));

        let mut with_artifact = req.clone();
        with_artifact.body_artifact = Some(serde_json::json!({"path": "/tmp/body"}));
        let err2 = validate_request_body_artifact(&dispatcher, &with_artifact)
            .expect_err("should fail with stub + artifact");
        assert!(err2.to_string().contains("ask body stub exceeds 4 KiB"));

        let small = envelope("hi");
        validate_request_body_artifact(&dispatcher, &small).unwrap();
    }

    #[test]
    fn test_attempt_sort_key_orders_by_retry_index() {
        let a = AttemptRecord {
            attempt_id: "a".to_string(),
            message_id: "m".to_string(),
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            job_id: "j".to_string(),
            retry_index: 1,
            health_snapshot_ref: None,
            started_at: "t".to_string(),
            updated_at: "t".to_string(),
            attempt_state: AttemptState::Completed,
        };
        let b = AttemptRecord {
            retry_index: 2,
            ..a.clone()
        };
        assert!(attempt_sort_key(&b) > attempt_sort_key(&a));
    }

    #[test]
    fn test_latest_attempts_by_agent_picks_latest() {
        let dispatcher = TestDispatcher::new(vec![]);
        dispatcher.with_attempt(AttemptRecord {
            attempt_id: "a1".to_string(),
            message_id: "m1".to_string(),
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            job_id: "j1".to_string(),
            retry_index: 0,
            health_snapshot_ref: None,
            started_at: "t1".to_string(),
            updated_at: "t1".to_string(),
            attempt_state: AttemptState::Completed,
        });
        dispatcher.with_attempt(AttemptRecord {
            attempt_id: "a2".to_string(),
            message_id: "m1".to_string(),
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            job_id: "j2".to_string(),
            retry_index: 1,
            health_snapshot_ref: None,
            started_at: "t2".to_string(),
            updated_at: "t2".to_string(),
            attempt_state: AttemptState::Completed,
        });
        let by_agent = latest_attempts_by_agent(&dispatcher, "m1");
        assert_eq!(by_agent.len(), 1);
        assert_eq!(by_agent["claude"].attempt_id, "a2");
    }

    #[test]
    fn test_resolve_retry_attempt_falls_back_to_job_id() {
        let dispatcher = TestDispatcher::new(vec![]);
        dispatcher.with_attempt(AttemptRecord {
            attempt_id: "a1".to_string(),
            message_id: "m1".to_string(),
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            job_id: "job-99".to_string(),
            retry_index: 0,
            health_snapshot_ref: None,
            started_at: "t".to_string(),
            updated_at: "t".to_string(),
            attempt_state: AttemptState::Completed,
        });
        assert_eq!(
            resolve_retry_attempt(&dispatcher, "claude")
                .unwrap()
                .attempt_id,
            "a1"
        );
        assert_eq!(
            resolve_retry_attempt(&dispatcher, "job-99")
                .unwrap()
                .attempt_id,
            "a1"
        );
        assert!(resolve_retry_attempt(&dispatcher, "missing").is_err());
    }

    #[test]
    fn test_plan_agent_submission_creates_drafts() {
        let dispatcher = TestDispatcher::new(vec!["claude".to_string(), "gemini".to_string()])
            .with_spec("claude", "claude-provider")
            .with_spec("gemini", "gemini-provider");

        let mut req = envelope("hello");
        req.delivery_scope = DeliveryScope::Broadcast;
        req.to_agent = "all".to_string();

        let plan = plan_agent_submission(&dispatcher, &req).unwrap();
        assert_eq!(plan.drafts.len(), 2);
        assert!(plan.submission_id.is_some());
        assert_eq!(plan.target_scope, Some("all".to_string()));
        assert_eq!(plan.drafts[0].target_kind, TargetKind::Agent);
        assert_eq!(plan.drafts[0].request.delivery_scope, DeliveryScope::Agent);
    }

    #[test]
    fn test_plan_message_resubmission_requires_terminal_attempts() {
        let dispatcher = TestDispatcher::new(vec![]);
        let message = MessageRecord {
            message_id: "m1".to_string(),
            origin_message_id: None,
            from_actor: "user".to_string(),
            target_scope: "single".to_string(),
            target_agents: vec!["claude".to_string()],
            message_class: "ask".to_string(),
            reply_policy: Value::Object(Default::default()),
            retry_policy: Value::Object(Default::default()),
            priority: 100,
            payload_ref: None,
            submission_id: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            message_state: ccbr_mailbox::models::MessageState::Created,
        };
        let dispatcher = dispatcher
            .with_spec("claude", "claude-provider")
            .with_message("m1", message);

        // No attempts -> error.
        assert!(plan_message_resubmission(&dispatcher, "m1").is_err());

        // Active attempt -> error.
        dispatcher.with_attempt(AttemptRecord {
            attempt_id: "a1".to_string(),
            message_id: "m1".to_string(),
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            job_id: "j1".to_string(),
            retry_index: 0,
            health_snapshot_ref: None,
            started_at: "t".to_string(),
            updated_at: "t".to_string(),
            attempt_state: AttemptState::Running,
        });
        assert!(plan_message_resubmission(&dispatcher, "m1").is_err());

        // Terminal attempt + matching job -> success.
        dispatcher
            .attempts
            .borrow_mut()
            .last_mut()
            .unwrap()
            .attempt_state = AttemptState::Completed;
        dispatcher.with_job(JobRecord {
            job_id: "j1".to_string(),
            submission_id: None,
            agent_name: "claude".to_string(),
            provider: "claude-provider".to_string(),
            request: envelope("orig"),
            status: JobStatus::Completed,
            terminal_decision: Some(Value::Object(Default::default())),
            cancel_requested_at: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            workspace_path: None,
            target_kind: TargetKind::Agent,
            target_name: "claude".to_string(),
            provider_instance: None,
            provider_options: Value::Object(Default::default()),
        });
        let plan = plan_message_resubmission(&dispatcher, "m1").unwrap();
        assert_eq!(plan.origin_message_id, Some("m1".to_string()));
        assert_eq!(plan.drafts.len(), 1);
    }
}
