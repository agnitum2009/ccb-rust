//! Mirrors Python `lib/ccbrd/services/dispatcher_runtime/reply_delivery_runtime/claims.py`.
//!
//! Claims a reply-delivery job by matching it to the agent's current mailbox
//! head and attempting to claim the underlying inbound event.

use ccbr_jobs::models::{JobRecord, JobStatus};
use ccbr_mailbox::models::InboundEventRecord;
use ccbr_mailbox::reply_payloads::delivery_job_id_from_payload;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const REPLY_DELIVERY_MESSAGE_TYPE: &str = "reply_delivery";
pub const REPLY_DELIVERY_PROVIDER_OPTION: &str = "reply_delivery";
pub const REPLY_DELIVERY_INBOUND_EVENT_OPTION: &str = "reply_delivery_inbound_event_id";

const PENDING_JOB_STATUSES: &[JobStatus] = &[JobStatus::Accepted];

// ---------------------------------------------------------------------------
// Dispatcher capability trait
// ---------------------------------------------------------------------------

pub trait Dispatcher {
    fn head_reply_event(&self, agent_name: &str) -> Option<InboundEventRecord>;
    fn get_job(&self, job_id: &str) -> Option<JobRecord>;
    fn claim_reply_delivery(
        &self,
        agent_name: &str,
        inbound_event_id: &str,
        started_at: &str,
    ) -> bool;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the job ids that can be claimed for reply delivery right now.
///
/// Mirrors Python `claimable_reply_delivery_job_ids`.
pub fn claimable_reply_delivery_job_ids<D: Dispatcher>(
    dispatcher: &D,
    agent_name: &str,
) -> Vec<String> {
    let head = match dispatcher.head_reply_event(agent_name) {
        Some(h) => h,
        None => return Vec::new(),
    };
    let job_id = match delivery_job_id_from_payload(head.payload_ref.as_deref()) {
        Some(j) => j,
        None => return Vec::new(),
    };
    let current = match dispatcher.get_job(&job_id) {
        Some(j) => j,
        None => return Vec::new(),
    };
    if !PENDING_JOB_STATUSES.contains(&current.status) {
        return Vec::new();
    }
    vec![job_id]
}

/// Claim the start of a reply-delivery job.
///
/// Mirrors Python `claim_reply_delivery_start`.
pub fn claim_reply_delivery_start<D: Dispatcher>(
    dispatcher: &D,
    job: &JobRecord,
    started_at: &str,
) -> bool {
    if !is_reply_delivery_job(job) {
        return true;
    }
    let inbound_event_id = match reply_delivery_inbound_event_id(job) {
        Some(id) => id,
        None => return false,
    };
    let head = match dispatcher.head_reply_event(&job.agent_name) {
        Some(h) => h,
        None => return false,
    };
    if head.inbound_event_id != inbound_event_id {
        return false;
    }
    dispatcher.claim_reply_delivery(&job.agent_name, &inbound_event_id, started_at)
}

/// Predicate: is this job a reply-delivery job?
///
/// Mirrors Python `is_reply_delivery_job`.
pub fn is_reply_delivery_job(job: &JobRecord) -> bool {
    if job
        .request
        .message_type
        .trim()
        .eq_ignore_ascii_case(REPLY_DELIVERY_MESSAGE_TYPE)
    {
        return true;
    }
    if let Some(opts) = job.provider_options.as_object() {
        if let Some(value) = opts.get(REPLY_DELIVERY_PROVIDER_OPTION) {
            return value.as_bool().unwrap_or(false);
        }
    }
    false
}

/// Extract the inbound event id stored in a reply-delivery job's options.
///
/// Mirrors Python `reply_delivery_inbound_event_id`.
pub fn reply_delivery_inbound_event_id(job: &JobRecord) -> Option<String> {
    let value = job
        .provider_options
        .as_object()
        .and_then(|opts| opts.get(REPLY_DELIVERY_INBOUND_EVENT_OPTION));
    match value {
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(other) => {
            let text = other.to_string();
            let trimmed = text.trim_matches('"').trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_jobs::models::{DeliveryScope, MessageEnvelope, TargetKind};
    use ccbr_mailbox::models::{InboundEventStatus, InboundEventType};

    fn job_with_options(provider_options: Value) -> JobRecord {
        JobRecord {
            job_id: "job_1".to_string(),
            submission_id: None,
            agent_name: "claude".to_string(),
            provider: "claude-provider".to_string(),
            request: MessageEnvelope {
                project_id: "proj".to_string(),
                to_agent: "claude".to_string(),
                from_actor: "system".to_string(),
                body: "body".to_string(),
                task_id: None,
                reply_to: None,
                message_type: "task_request".to_string(),
                delivery_scope: DeliveryScope::Agent,
                silence_on_success: false,
                route_options: Value::Object(Default::default()),
                body_artifact: None,
            },
            status: JobStatus::Accepted,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            workspace_path: None,
            target_kind: TargetKind::Agent,
            target_name: "claude".to_string(),
            provider_instance: None,
            provider_options,
        }
    }

    struct TestDispatcher {
        head: Option<InboundEventRecord>,
        jobs: std::collections::HashMap<String, JobRecord>,
        claimed: std::cell::RefCell<Vec<(String, String)>>,
    }

    impl TestDispatcher {
        fn new(head: Option<InboundEventRecord>) -> Self {
            Self {
                head,
                jobs: std::collections::HashMap::new(),
                claimed: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn with_job(mut self, job: JobRecord) -> Self {
            self.jobs.insert(job.job_id.clone(), job);
            self
        }
    }

    impl Dispatcher for TestDispatcher {
        fn head_reply_event(&self, _agent_name: &str) -> Option<InboundEventRecord> {
            self.head.clone()
        }

        fn get_job(&self, job_id: &str) -> Option<JobRecord> {
            self.jobs.get(job_id).cloned()
        }

        fn claim_reply_delivery(
            &self,
            agent_name: &str,
            inbound_event_id: &str,
            _started_at: &str,
        ) -> bool {
            self.claimed
                .borrow_mut()
                .push((agent_name.to_string(), inbound_event_id.to_string()));
            true
        }
    }

    #[test]
    fn test_is_reply_delivery_job() {
        let mut job = job_with_options(Value::Object(Default::default()));
        assert!(!is_reply_delivery_job(&job));

        job.request.message_type = REPLY_DELIVERY_MESSAGE_TYPE.to_string();
        assert!(is_reply_delivery_job(&job));

        job.request.message_type = "task_request".to_string();
        job.provider_options = serde_json::json!({REPLY_DELIVERY_PROVIDER_OPTION: true});
        assert!(is_reply_delivery_job(&job));
    }

    #[test]
    fn test_reply_delivery_inbound_event_id() {
        let job = job_with_options(serde_json::json!({
            REPLY_DELIVERY_INBOUND_EVENT_OPTION: "evt_1"
        }));
        assert_eq!(
            reply_delivery_inbound_event_id(&job),
            Some("evt_1".to_string())
        );

        let empty = job_with_options(serde_json::json!({
            REPLY_DELIVERY_INBOUND_EVENT_OPTION: "  "
        }));
        assert_eq!(reply_delivery_inbound_event_id(&empty), None);
    }

    #[test]
    fn test_claimable_reply_delivery_job_ids() {
        let head = InboundEventRecord {
            inbound_event_id: "evt_1".to_string(),
            agent_name: "claude".to_string(),
            event_type: InboundEventType::TaskReply,
            message_id: "m1".to_string(),
            attempt_id: None,
            payload_ref: Some("reply:rep_1 delivery:job_1".to_string()),
            priority: 10,
            status: InboundEventStatus::Queued,
            created_at: "t".to_string(),
            started_at: None,
            finished_at: None,
        };
        let job = job_with_options(Value::Object(Default::default()));
        let dispatcher = TestDispatcher::new(Some(head)).with_job(job);
        let ids = claimable_reply_delivery_job_ids(&dispatcher, "claude");
        assert_eq!(ids, vec!["job_1".to_string()]);
    }

    #[test]
    fn test_claim_reply_delivery_start_claims_when_head_matches() {
        let head = InboundEventRecord {
            inbound_event_id: "evt_1".to_string(),
            agent_name: "claude".to_string(),
            event_type: InboundEventType::TaskReply,
            message_id: "m1".to_string(),
            attempt_id: None,
            payload_ref: Some("reply:rep_1 delivery:job_1".to_string()),
            priority: 10,
            status: InboundEventStatus::Queued,
            created_at: "t".to_string(),
            started_at: None,
            finished_at: None,
        };
        let job = job_with_options(serde_json::json!({
            REPLY_DELIVERY_PROVIDER_OPTION: true,
            REPLY_DELIVERY_INBOUND_EVENT_OPTION: "evt_1"
        }));
        let dispatcher = TestDispatcher::new(Some(head));
        assert!(claim_reply_delivery_start(&dispatcher, &job, "now"));
        assert_eq!(
            dispatcher.claimed.borrow().clone(),
            vec![("claude".to_string(), "evt_1".to_string())]
        );
    }

    #[test]
    fn test_claim_reply_delivery_start_false_when_head_mismatch() {
        let head = InboundEventRecord {
            inbound_event_id: "evt_other".to_string(),
            agent_name: "claude".to_string(),
            event_type: InboundEventType::TaskReply,
            message_id: "m1".to_string(),
            attempt_id: None,
            payload_ref: Some("reply:rep_1 delivery:job_1".to_string()),
            priority: 10,
            status: InboundEventStatus::Queued,
            created_at: "t".to_string(),
            started_at: None,
            finished_at: None,
        };
        let job = job_with_options(serde_json::json!({
            REPLY_DELIVERY_PROVIDER_OPTION: true,
            REPLY_DELIVERY_INBOUND_EVENT_OPTION: "evt_1"
        }));
        let dispatcher = TestDispatcher::new(Some(head));
        assert!(!claim_reply_delivery_start(&dispatcher, &job, "now"));
    }

    #[test]
    fn test_non_reply_delivery_job_claims_trivially() {
        let dispatcher = TestDispatcher::new(None);
        let job = job_with_options(Value::Object(Default::default()));
        assert!(claim_reply_delivery_start(&dispatcher, &job, "now"));
        assert!(dispatcher.claimed.borrow().is_empty());
    }
}
