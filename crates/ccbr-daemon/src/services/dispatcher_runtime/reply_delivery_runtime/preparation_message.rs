//! Mirrors Python `lib/ccbrd/services/dispatcher_runtime/reply_delivery_runtime/preparation_message.py`.
//!
//! Builds the reply-delivery job, message, and attempt records that carry a
//! provider reply back to the target agent.

use ccbr_jobs::models::{DeliveryScope, JobRecord, JobStatus, MessageEnvelope, TargetKind};
use ccbr_mailbox::models::{
    AttemptRecord, AttemptState, InboundEventRecord, InboundEventStatus, MessageRecord,
    MessageState, ReplyRecord, ReplyTerminalStatus,
};
use ccbr_mailbox::reply_payloads::compose_reply_payload;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const REPLY_DELIVERY_MESSAGE_TYPE: &str = "reply_delivery";
pub const REPLY_DELIVERY_PROVIDER_OPTION: &str = "reply_delivery";
pub const REPLY_DELIVERY_INBOUND_EVENT_OPTION: &str = "reply_delivery_inbound_event_id";
pub const REPLY_DELIVERY_REPLY_ID_OPTION: &str = "reply_delivery_reply_id";

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Minimal agent spec needed to build a reply-delivery job.
#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub provider: String,
}

/// Minimal agent runtime snapshot.
#[derive(Debug, Clone, Default)]
pub struct AgentRuntime {
    pub project_id: Option<String>,
    pub workspace_path: Option<String>,
}

/// A draft job before it is recorded, matching the Python `_JobDraft` shape.
#[derive(Debug, Clone)]
pub struct JobDraft {
    pub agent_name: String,
    pub provider: String,
    pub request: MessageEnvelope,
    pub target_kind: TargetKind,
    pub target_name: String,
    pub provider_instance: Option<String>,
    pub provider_options: Option<Value>,
    pub workspace_path: Option<String>,
}

/// The message + attempt pair created for a reply delivery.
#[derive(Debug, Clone)]
pub struct ReplyDeliveryMessage {
    pub message_record: MessageRecord,
    pub attempt_record: AttemptRecord,
}

// ---------------------------------------------------------------------------
// Dispatcher capability trait
// ---------------------------------------------------------------------------

pub trait Dispatcher {
    fn clock(&self) -> String;
    fn new_id(&self, prefix: &str) -> String;
    fn agent_spec(&self, agent_name: &str) -> Option<AgentSpec>;
    fn agent_runtime(&self, agent_name: &str) -> Option<AgentRuntime>;
    fn source_job_for_reply(&self, reply: &ReplyRecord) -> Option<JobRecord>;
    fn append_job(&self, job: &JobRecord);
    fn append_message_record(&self, record: &MessageRecord);
    fn append_attempt_record(&self, record: &AttemptRecord);
    fn append_event(&self, job: &JobRecord, event_type: &str, payload: Value, timestamp: &str);
    fn rewrite_reply_head(
        &self,
        head: &InboundEventRecord,
        reply_id: &str,
        delivery_job_id: Option<&str>,
        status: InboundEventStatus,
        updated_at: &str,
        clear_progress: bool,
    );
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Resolve a workspace path for the reply-delivery job.
///
/// Mirrors Python `resolve_workspace_path`.
pub fn resolve_workspace_path(
    _dispatcher: &impl Dispatcher,
    agent_name: &str,
    runtime: Option<&AgentRuntime>,
) -> String {
    if let Some(r) = runtime {
        if let Some(path) = &r.workspace_path {
            return path.clone();
        }
    }
    format!("/workspace/{agent_name}")
}

/// Build the request envelope that delivers a reply to an agent.
///
/// Mirrors Python `build_reply_delivery_request`.
pub fn build_reply_delivery_request<D: Dispatcher>(
    dispatcher: &D,
    reply: &ReplyRecord,
    project_id: &str,
    agent_name: &str,
) -> MessageEnvelope {
    MessageEnvelope {
        project_id: project_id.to_string(),
        to_agent: agent_name.to_string(),
        from_actor: "system".to_string(),
        body: format_reply_delivery_body(dispatcher, reply),
        task_id: Some(format!("reply:{}", reply.reply_id)),
        reply_to: None,
        message_type: REPLY_DELIVERY_MESSAGE_TYPE.to_string(),
        delivery_scope: DeliveryScope::Agent,
        silence_on_success: false,
        route_options: Value::Object(Default::default()),
        body_artifact: None,
    }
}

/// Build (but do not persist) the reply-delivery message and attempt records.
///
/// Mirrors the construction half of Python `append_reply_delivery_message`.
pub fn build_reply_delivery_message<D: Dispatcher>(
    dispatcher: &D,
    reply: &ReplyRecord,
    agent_name: &str,
    accepted_at: &str,
    spec: &AgentSpec,
    job_id: &str,
) -> ReplyDeliveryMessage {
    let message_id = dispatcher.new_id("msg");
    let attempt_id = dispatcher.new_id("att");
    let message = MessageRecord {
        message_id,
        origin_message_id: Some(reply.message_id.clone()),
        from_actor: "system".to_string(),
        target_scope: "single".to_string(),
        target_agents: vec![agent_name.to_string()],
        message_class: REPLY_DELIVERY_MESSAGE_TYPE.to_string(),
        reply_policy: serde_json::json!({"mode": "none", "expected_reply_count": 0}),
        retry_policy: serde_json::json!({"mode": "manual"}),
        priority: 10,
        payload_ref: Some(format!("reply:{}", reply.reply_id)),
        submission_id: None,
        created_at: accepted_at.to_string(),
        updated_at: accepted_at.to_string(),
        message_state: MessageState::Queued,
    };
    let attempt = AttemptRecord {
        attempt_id,
        message_id: message.message_id.clone(),
        agent_name: agent_name.to_string(),
        provider: spec.provider.clone(),
        job_id: job_id.to_string(),
        retry_index: 0,
        health_snapshot_ref: None,
        started_at: accepted_at.to_string(),
        updated_at: accepted_at.to_string(),
        attempt_state: AttemptState::Pending,
    };
    ReplyDeliveryMessage {
        message_record: message,
        attempt_record: attempt,
    }
}

/// Persist the reply-delivery message and attempt records.
///
/// Mirrors Python `append_reply_delivery_message`.
pub fn append_reply_delivery_message<D: Dispatcher>(
    dispatcher: &D,
    reply: &ReplyRecord,
    agent_name: &str,
    accepted_at: &str,
    spec: &AgentSpec,
    job_id: &str,
) {
    let bundle =
        build_reply_delivery_message(dispatcher, reply, agent_name, accepted_at, spec, job_id);
    dispatcher.append_message_record(&bundle.message_record);
    dispatcher.append_attempt_record(&bundle.attempt_record);
}

/// Record a `reply_delivery_scheduled` event on the delivery job.
///
/// Mirrors Python `record_reply_delivery_scheduled`.
pub fn record_reply_delivery_scheduled<D: Dispatcher>(
    dispatcher: &D,
    job: &JobRecord,
    inbound_event_id: &str,
    reply_id: &str,
    accepted_at: &str,
) {
    dispatcher.append_event(
        job,
        "reply_delivery_scheduled",
        serde_json::json!({
            "inbound_event_id": inbound_event_id,
            "reply_id": reply_id,
        }),
        accepted_at,
    );
}

/// Build, record, and enqueue a reply-delivery job for a pending reply.
///
/// Mirrors Python `build_reply_delivery_job`.
pub fn build_reply_delivery_job<D: Dispatcher>(
    dispatcher: &D,
    agent_name: &str,
    head: &InboundEventRecord,
    reply: &ReplyRecord,
    accepted_at: &str,
    project_id: &str,
) -> Option<JobRecord> {
    let spec = dispatcher.agent_spec(agent_name)?;
    let runtime = dispatcher.agent_runtime(agent_name);
    let workspace_path = resolve_workspace_path(dispatcher, agent_name, runtime.as_ref());
    let request = build_reply_delivery_request(dispatcher, reply, project_id, agent_name);
    let job_id = dispatcher.new_id("job");

    let mut provider_options = serde_json::Map::new();
    provider_options.insert("no_wrap".to_string(), Value::Bool(true));
    provider_options.insert(
        REPLY_DELIVERY_PROVIDER_OPTION.to_string(),
        Value::Bool(true),
    );
    provider_options.insert(
        REPLY_DELIVERY_INBOUND_EVENT_OPTION.to_string(),
        head.inbound_event_id.clone().into(),
    );
    provider_options.insert(
        REPLY_DELIVERY_REPLY_ID_OPTION.to_string(),
        reply.reply_id.clone().into(),
    );

    let draft = JobDraft {
        agent_name: agent_name.to_string(),
        provider: spec.provider.clone(),
        request,
        target_kind: TargetKind::Agent,
        target_name: agent_name.to_string(),
        provider_instance: None,
        provider_options: Some(Value::Object(provider_options)),
        workspace_path: Some(workspace_path),
    };

    let (job, status) = build_job_record(dispatcher, &draft, &job_id, None, accepted_at);
    dispatcher.append_job(&job);
    enqueue_submitted_job(dispatcher, &job, &status, accepted_at);
    append_reply_delivery_message(dispatcher, reply, agent_name, accepted_at, &spec, &job_id);

    rewrite_reply_head(
        dispatcher,
        head,
        &reply.reply_id,
        Some(&job_id),
        InboundEventStatus::Queued,
        accepted_at,
        true,
    );
    record_reply_delivery_scheduled(
        dispatcher,
        &job,
        &head.inbound_event_id,
        &reply.reply_id,
        accepted_at,
    );

    Some(job)
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format the body text for a reply-delivery request.
///
/// Mirrors Python `format_reply_delivery_body`.
pub fn format_reply_delivery_body<D: Dispatcher>(dispatcher: &D, reply: &ReplyRecord) -> String {
    let source_job = dispatcher.source_job_for_reply(reply);
    if is_heartbeat_notice(reply) {
        format_heartbeat_delivery_body(reply, source_job.as_ref())
    } else {
        let header = reply_header(reply, source_job.as_ref());
        let body = if reply.reply.is_empty() {
            "(empty reply)"
        } else {
            &reply.reply
        };
        let mut text = header.join(" ");
        text.push('\n');
        text.push('\n');
        text.push_str(body);
        text.trim_end().to_string()
    }
}

fn is_heartbeat_notice(reply: &ReplyRecord) -> bool {
    reply
        .diagnostics
        .get("notice_kind")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().eq_ignore_ascii_case("heartbeat"))
        .unwrap_or(false)
}

fn reply_header(reply: &ReplyRecord, source_job: Option<&JobRecord>) -> Vec<String> {
    let mut header = vec![
        "CCBR_REPLY".to_string(),
        format!("from={}", reply.agent_name),
        format!("reply={}", reply.reply_id),
        format!("status={}", format_status(reply.terminal_status)),
    ];
    if let Some(job) = source_job {
        header.push(format!("job={}", job.job_id));
        if let Some(task_id) = job.request.task_id.as_deref().map(|s| s.trim()) {
            if !task_id.is_empty() {
                header.push(format!("task={task_id}"));
            }
        }
    }
    header
}

fn format_heartbeat_delivery_body(reply: &ReplyRecord, source_job: Option<&JobRecord>) -> String {
    let mut lines = vec![heartbeat_header(reply, source_job)];
    let body = reply.reply.trim();
    if body.is_empty() {
        lines.push("".to_string());
        lines.push("(empty notice)".to_string());
    } else {
        lines.push("".to_string());
        lines.push(body.to_string());
    }
    lines.join("\n").trim_end().to_string()
}

fn heartbeat_header(reply: &ReplyRecord, source_job: Option<&JobRecord>) -> String {
    let diagnostics = &reply.diagnostics;
    let mut header = format!(
        "CCBR_NOTICE kind=heartbeat from={} reply={}",
        reply.agent_name, reply.reply_id
    );
    if let Some(job_id) = heartbeat_job_id(diagnostics, source_job) {
        header.push_str(&format!(" job={job_id}"));
    }
    if let Some(task_id) = heartbeat_task_id(source_job) {
        header.push_str(&format!(" task={task_id}"));
    }
    if let Some(last_progress) = diagnostics
        .get("last_progress_at")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
    {
        if !last_progress.is_empty() {
            header.push_str(&format!(" last_progress={last_progress}"));
        }
    }
    if let Some(seconds) = diagnostics.get("heartbeat_silence_seconds") {
        header.push_str(&format!(" silent_for={}", format_silence_seconds(seconds)));
    }
    header
}

fn heartbeat_job_id(diagnostics: &Value, source_job: Option<&JobRecord>) -> Option<String> {
    if let Some(id) = diagnostics
        .get("job_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    source_job.map(|j| j.job_id.clone())
}

fn heartbeat_task_id(source_job: Option<&JobRecord>) -> Option<String> {
    source_job.and_then(|j| {
        j.request
            .task_id
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    })
}

/// Format a heartbeat silence duration.
///
/// Mirrors Python `format_silence_seconds`.
pub fn format_silence_seconds(value: &Value) -> String {
    if let Some(n) = value.as_u64() {
        return format!("{n}s");
    }
    if let Some(f) = value.as_f64() {
        return format!("{}s", f.round() as i64);
    }
    if let Some(s) = value.as_str() {
        if let Ok(f) = s.parse::<f64>() {
            return format!("{}s", f.round() as i64);
        }
        return s.to_string();
    }
    value.to_string()
}

fn format_status(status: ReplyTerminalStatus) -> String {
    format!("{:?}", status).to_lowercase()
}

// ---------------------------------------------------------------------------
// Job recording helpers
// ---------------------------------------------------------------------------

fn build_job_record<D: Dispatcher>(
    _dispatcher: &D,
    draft: &JobDraft,
    job_id: &str,
    submission_id: Option<&str>,
    accepted_at: &str,
) -> (JobRecord, String) {
    let job = JobRecord {
        job_id: job_id.to_string(),
        submission_id: submission_id.map(|s| s.to_string()),
        agent_name: draft.agent_name.clone(),
        provider: draft.provider.clone(),
        request: draft.request.clone(),
        status: JobStatus::Accepted,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: accepted_at.to_string(),
        updated_at: accepted_at.to_string(),
        workspace_path: draft.workspace_path.clone(),
        target_kind: draft.target_kind,
        target_name: draft.target_name.clone(),
        provider_instance: draft.provider_instance.clone(),
        provider_options: draft
            .provider_options
            .clone()
            .unwrap_or_else(|| Value::Object(Default::default())),
    };
    (job, "accepted".to_string())
}

fn enqueue_submitted_job<D: Dispatcher>(
    _dispatcher: &D,
    _job: &JobRecord,
    _status: &str,
    _accepted_at: &str,
) {
    // Runtime-state enqueueing is out of scope for this stub.
}

fn rewrite_reply_head<D: Dispatcher>(
    dispatcher: &D,
    head: &InboundEventRecord,
    reply_id: &str,
    delivery_job_id: Option<&str>,
    status: InboundEventStatus,
    updated_at: &str,
    clear_progress: bool,
) {
    let payload_ref = compose_reply_payload(reply_id, delivery_job_id);
    dispatcher.rewrite_reply_head(
        head,
        reply_id,
        Some(&payload_ref),
        status,
        updated_at,
        clear_progress,
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn reply() -> ReplyRecord {
        ReplyRecord {
            reply_id: "rep_1".to_string(),
            message_id: "m1".to_string(),
            attempt_id: "a1".to_string(),
            agent_name: "claude".to_string(),
            terminal_status: ReplyTerminalStatus::Completed,
            reply: "done".to_string(),
            reply_artifact: None,
            diagnostics: Value::Object(Default::default()),
            finished_at: "t".to_string(),
        }
    }

    fn job(job_id: &str, task_id: Option<&str>) -> JobRecord {
        JobRecord {
            job_id: job_id.to_string(),
            submission_id: None,
            agent_name: "claude".to_string(),
            provider: "claude-provider".to_string(),
            request: MessageEnvelope {
                project_id: "proj".to_string(),
                to_agent: "claude".to_string(),
                from_actor: "user".to_string(),
                body: "orig".to_string(),
                task_id: task_id.map(|s| s.to_string()),
                reply_to: None,
                message_type: "ask".to_string(),
                delivery_scope: DeliveryScope::Agent,
                silence_on_success: false,
                route_options: Value::Object(Default::default()),
                body_artifact: None,
            },
            status: JobStatus::Completed,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            workspace_path: None,
            target_kind: TargetKind::Agent,
            target_name: "claude".to_string(),
            provider_instance: None,
            provider_options: Value::Object(Default::default()),
        }
    }

    #[derive(Default)]
    struct TestDispatcher {
        source_job: RefCell<Option<JobRecord>>,
        records: RefCell<Vec<Recorded>>,
        next_id: RefCell<u32>,
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code, clippy::large_enum_variant)]
    enum Recorded {
        Job(JobRecord),
        Message(MessageRecord),
        Attempt(AttemptRecord),
        Event(String, Value), // (event_type, payload)
        RewrittenHead(String, String, InboundEventStatus), // (reply_id, payload_ref, status)
    }

    impl TestDispatcher {
        fn with_source_job(self, j: JobRecord) -> Self {
            *self.source_job.borrow_mut() = Some(j);
            self
        }

        fn recorded_jobs(&self) -> Vec<JobRecord> {
            self.records
                .borrow()
                .iter()
                .filter_map(|r| match r {
                    Recorded::Job(j) => Some(j.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    impl Dispatcher for TestDispatcher {
        fn clock(&self) -> String {
            "2025-01-01T00:00:00Z".to_string()
        }

        fn new_id(&self, prefix: &str) -> String {
            let n = *self.next_id.borrow();
            *self.next_id.borrow_mut() += 1;
            format!("{prefix}_{n}")
        }

        fn agent_spec(&self, _agent_name: &str) -> Option<AgentSpec> {
            Some(AgentSpec {
                provider: "claude-provider".to_string(),
            })
        }

        fn agent_runtime(&self, _agent_name: &str) -> Option<AgentRuntime> {
            None
        }

        fn source_job_for_reply(&self, _reply: &ReplyRecord) -> Option<JobRecord> {
            self.source_job.borrow().clone()
        }

        fn append_job(&self, job: &JobRecord) {
            self.records.borrow_mut().push(Recorded::Job(job.clone()));
        }

        fn append_message_record(&self, record: &MessageRecord) {
            self.records
                .borrow_mut()
                .push(Recorded::Message(record.clone()));
        }

        fn append_attempt_record(&self, record: &AttemptRecord) {
            self.records
                .borrow_mut()
                .push(Recorded::Attempt(record.clone()));
        }

        fn append_event(
            &self,
            _job: &JobRecord,
            event_type: &str,
            payload: Value,
            _timestamp: &str,
        ) {
            self.records
                .borrow_mut()
                .push(Recorded::Event(event_type.to_string(), payload));
        }

        fn rewrite_reply_head(
            &self,
            _head: &InboundEventRecord,
            reply_id: &str,
            delivery_job_id: Option<&str>,
            status: InboundEventStatus,
            _updated_at: &str,
            _clear_progress: bool,
        ) {
            self.records.borrow_mut().push(Recorded::RewrittenHead(
                reply_id.to_string(),
                delivery_job_id.unwrap_or("").to_string(),
                status,
            ));
        }
    }

    #[test]
    fn test_format_reply_delivery_body() {
        let dispatcher = TestDispatcher::default().with_source_job(job("j1", Some("task-1")));
        let text = format_reply_delivery_body(&dispatcher, &reply());
        assert!(text
            .starts_with("CCBR_REPLY from=claude reply=rep_1 status=completed job=j1 task=task-1"));
        assert!(text.ends_with("done"));
    }

    #[test]
    fn test_format_heartbeat_delivery_body() {
        let mut r = reply();
        r.diagnostics =
            serde_json::json!({"notice_kind": "heartbeat", "heartbeat_silence_seconds": 42.7});
        r.reply = "still alive".to_string();
        let dispatcher = TestDispatcher::default().with_source_job(job("j1", Some("task-1")));
        let text = format_reply_delivery_body(&dispatcher, &r);
        assert!(text.starts_with(
            "CCBR_NOTICE kind=heartbeat from=claude reply=rep_1 job=j1 task=task-1 silent_for=43s"
        ));
        assert!(text.contains("still alive"));
    }

    #[test]
    fn test_format_silence_seconds() {
        assert_eq!(format_silence_seconds(&serde_json::json!(42)), "42s");
        assert_eq!(format_silence_seconds(&serde_json::json!(42.7)), "43s");
        assert_eq!(format_silence_seconds(&serde_json::json!("abc")), "abc");
        assert_eq!(format_silence_seconds(&serde_json::json!("12.3")), "12s");
    }

    #[test]
    fn test_build_reply_delivery_request() {
        let dispatcher = TestDispatcher::default();
        let req = build_reply_delivery_request(&dispatcher, &reply(), "proj", "claude");
        assert_eq!(req.project_id, "proj");
        assert_eq!(req.to_agent, "claude");
        assert_eq!(req.from_actor, "system");
        assert_eq!(req.message_type, REPLY_DELIVERY_MESSAGE_TYPE);
        assert_eq!(req.task_id, Some("reply:rep_1".to_string()));
    }

    #[test]
    fn test_build_reply_delivery_job_records_everything() {
        let dispatcher = TestDispatcher::default();
        let head = InboundEventRecord {
            inbound_event_id: "evt_1".to_string(),
            agent_name: "claude".to_string(),
            event_type: ccbr_mailbox::models::InboundEventType::TaskReply,
            message_id: "m1".to_string(),
            attempt_id: None,
            payload_ref: Some("reply:rep_1".to_string()),
            priority: 10,
            status: InboundEventStatus::Queued,
            created_at: "t".to_string(),
            started_at: None,
            finished_at: None,
        };
        let job = build_reply_delivery_job(&dispatcher, "claude", &head, &reply(), "t", "proj")
            .expect("job created");
        assert_eq!(job.agent_name, "claude");
        assert_eq!(job.request.message_type, REPLY_DELIVERY_MESSAGE_TYPE);

        let jobs = dispatcher.recorded_jobs();
        assert_eq!(jobs.len(), 1);
        assert!(dispatcher
            .records
            .borrow()
            .iter()
            .any(|r| matches!(r, Recorded::Message(_))));
        assert!(dispatcher
            .records
            .borrow()
            .iter()
            .any(|r| matches!(r, Recorded::Attempt(_))));
        assert!(dispatcher.records.borrow().iter().any(
            |r| matches!(r, Recorded::Event(etype, _) if etype == "reply_delivery_scheduled")
        ));
        assert!(dispatcher
            .records
            .borrow()
            .iter()
            .any(|r| matches!(r, Recorded::RewrittenHead(_, _, InboundEventStatus::Queued))));
    }

    #[test]
    fn test_resolve_workspace_path_prefers_runtime() {
        let dispatcher = TestDispatcher::default();
        let runtime = AgentRuntime {
            workspace_path: Some("/custom".to_string()),
            project_id: None,
        };
        assert_eq!(
            resolve_workspace_path(&dispatcher, "claude", Some(&runtime)),
            "/custom"
        );
        assert_eq!(
            resolve_workspace_path(&dispatcher, "claude", None),
            "/workspace/claude"
        );
    }
}
