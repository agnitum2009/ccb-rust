//! Mirrors Python `lib/ccbrd/services/dispatcher_runtime/reply_delivery_runtime/preparation_service.py`.
//!
//! Walks each configured agent, finds pending reply events at the head of the
//! mailbox, and prepares a reply-delivery job for each one.

use ccbr_jobs::models::JobRecord;
use ccbr_mailbox::models::{InboundEventRecord, ReplyRecord};
use ccbr_mailbox::reply_payloads::reply_id_from_payload;

// ---------------------------------------------------------------------------
// Dispatcher capability trait
// ---------------------------------------------------------------------------

/// Minimal dispatcher surface needed to prepare reply deliveries.
pub trait Dispatcher {
    fn config_agents(&self) -> &[String];
    fn head_reply_event(&self, agent_name: &str) -> Option<InboundEventRecord>;
    fn project_id_for_agent(&self, agent_name: &str) -> Option<String>;
    fn resolve_existing_delivery_job(
        &self,
        agent_name: &str,
        head: &InboundEventRecord,
        reply_id: &str,
    ) -> Option<InboundEventRecord>;
    fn get_reply(&self, reply_id: &str) -> Option<ReplyRecord>;
    fn build_reply_delivery_job(
        &self,
        agent_name: &str,
        head: &InboundEventRecord,
        reply: &ReplyRecord,
        accepted_at: &str,
        project_id: &str,
    ) -> Option<JobRecord>;
    fn clock(&self) -> String;
}

/// Alias requested by the task brief.
pub type ReplyDeliveryPreparation = JobRecord;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Prepare reply-delivery jobs for every configured agent.
///
/// Mirrors Python `prepare_reply_deliveries`.
pub fn prepare_reply_deliveries<D: Dispatcher>(dispatcher: &D) -> Vec<JobRecord> {
    let mut created = Vec::new();
    for agent_name in dispatcher.config_agents() {
        if let Some(job) = prepare_agent_reply_delivery(dispatcher, agent_name) {
            created.push(job);
        }
    }
    created
}

/// Prepare a single reply-delivery job for `agent_name` if a reply is pending.
///
/// Mirrors Python `prepare_agent_reply_delivery`.
pub fn prepare_agent_reply_delivery<D: Dispatcher>(
    dispatcher: &D,
    agent_name: &str,
) -> Option<JobRecord> {
    let head = dispatcher.head_reply_event(agent_name)?;
    let reply_id = head_reply_id(&head)?;

    let head = dispatcher.resolve_existing_delivery_job(agent_name, &head, &reply_id)?;

    let reply = dispatcher.get_reply(&reply_id)?;
    let accepted_at = dispatcher.clock();
    let project_id = dispatcher.project_id_for_agent(agent_name)?;

    dispatcher.build_reply_delivery_job(agent_name, &head, &reply, &accepted_at, &project_id)
}

/// Extract the reply id from a pending reply event head.
///
/// Mirrors Python `head_reply_id`.
pub fn head_reply_id(head: &InboundEventRecord) -> Option<String> {
    reply_id_from_payload(head.payload_ref.as_deref())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_mailbox::models::{InboundEventStatus, InboundEventType, ReplyTerminalStatus};

    struct TestDispatcher {
        agents: Vec<String>,
        head: Option<InboundEventRecord>,
        reply: Option<ReplyRecord>,
        project_id: Option<String>,
        resolved_head: Option<InboundEventRecord>,
        built_job: Option<JobRecord>,
    }

    impl TestDispatcher {
        fn simple() -> Self {
            Self {
                agents: vec!["claude".to_string()],
                head: Some(InboundEventRecord {
                    inbound_event_id: "evt_1".to_string(),
                    agent_name: "claude".to_string(),
                    event_type: InboundEventType::TaskReply,
                    message_id: "m1".to_string(),
                    attempt_id: None,
                    payload_ref: Some("reply:rep_1".to_string()),
                    priority: 10,
                    status: InboundEventStatus::Queued,
                    created_at: "t".to_string(),
                    started_at: None,
                    finished_at: None,
                }),
                reply: Some(ReplyRecord {
                    reply_id: "rep_1".to_string(),
                    message_id: "m1".to_string(),
                    attempt_id: "a1".to_string(),
                    agent_name: "claude".to_string(),
                    terminal_status: ReplyTerminalStatus::Completed,
                    reply: "done".to_string(),
                    reply_artifact: None,
                    diagnostics: serde_json::Value::Object(Default::default()),
                    finished_at: "t".to_string(),
                }),
                project_id: Some("proj".to_string()),
                resolved_head: None,
                built_job: None,
            }
        }

        fn with_job(mut self, job: JobRecord) -> Self {
            self.built_job = Some(job);
            self
        }
    }

    impl Dispatcher for TestDispatcher {
        fn config_agents(&self) -> &[String] {
            &self.agents
        }

        fn head_reply_event(&self, _agent_name: &str) -> Option<InboundEventRecord> {
            self.head.clone()
        }

        fn project_id_for_agent(&self, _agent_name: &str) -> Option<String> {
            self.project_id.clone()
        }

        fn resolve_existing_delivery_job(
            &self,
            _agent_name: &str,
            head: &InboundEventRecord,
            _reply_id: &str,
        ) -> Option<InboundEventRecord> {
            self.resolved_head.clone().or(Some(head.clone()))
        }

        fn get_reply(&self, _reply_id: &str) -> Option<ReplyRecord> {
            self.reply.clone()
        }

        fn build_reply_delivery_job(
            &self,
            _agent_name: &str,
            _head: &InboundEventRecord,
            _reply: &ReplyRecord,
            _accepted_at: &str,
            _project_id: &str,
        ) -> Option<JobRecord> {
            self.built_job.clone()
        }

        fn clock(&self) -> String {
            "2025-01-01T00:00:00Z".to_string()
        }
    }

    #[test]
    fn test_head_reply_id_extracts_id() {
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
        assert_eq!(head_reply_id(&head), Some("rep_1".to_string()));
    }

    #[test]
    fn test_prepare_agent_reply_delivery_creates_job() {
        let job = JobRecord {
            job_id: "job_1".to_string(),
            submission_id: None,
            agent_name: "claude".to_string(),
            provider: "claude-provider".to_string(),
            request: ccbr_jobs::models::MessageEnvelope {
                project_id: "proj".to_string(),
                to_agent: "claude".to_string(),
                from_actor: "system".to_string(),
                body: "CCBR_REPLY from=claude reply=rep_1 status=completed".to_string(),
                task_id: Some("reply:rep_1".to_string()),
                reply_to: None,
                message_type: super::super::preparation_message::REPLY_DELIVERY_MESSAGE_TYPE
                    .to_string(),
                delivery_scope: ccbr_jobs::models::DeliveryScope::Agent,
                silence_on_success: false,
                route_options: serde_json::Value::Object(Default::default()),
                body_artifact: None,
            },
            status: ccbr_jobs::models::JobStatus::Accepted,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            workspace_path: None,
            target_kind: ccbr_jobs::models::TargetKind::Agent,
            target_name: "claude".to_string(),
            provider_instance: None,
            provider_options: serde_json::Value::Object(Default::default()),
        };
        let dispatcher = TestDispatcher::simple().with_job(job.clone());
        let result = prepare_agent_reply_delivery(&dispatcher, "claude").unwrap();
        assert_eq!(result.job_id, job.job_id);
    }

    #[test]
    fn test_prepare_agent_reply_delivery_missing_head_returns_none() {
        let mut dispatcher = TestDispatcher::simple();
        dispatcher.head = None;
        assert!(prepare_agent_reply_delivery(&dispatcher, "claude").is_none());
    }

    #[test]
    fn test_prepare_reply_deliveries_iterates_all_agents() {
        let job = JobRecord {
            job_id: "job_1".to_string(),
            submission_id: None,
            agent_name: "claude".to_string(),
            provider: "claude-provider".to_string(),
            request: ccbr_jobs::models::MessageEnvelope {
                project_id: "proj".to_string(),
                to_agent: "claude".to_string(),
                from_actor: "system".to_string(),
                body: "body".to_string(),
                task_id: None,
                reply_to: None,
                message_type: "reply_delivery".to_string(),
                delivery_scope: ccbr_jobs::models::DeliveryScope::Agent,
                silence_on_success: false,
                route_options: serde_json::Value::Object(Default::default()),
                body_artifact: None,
            },
            status: ccbr_jobs::models::JobStatus::Accepted,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            workspace_path: None,
            target_kind: ccbr_jobs::models::TargetKind::Agent,
            target_name: "claude".to_string(),
            provider_instance: None,
            provider_options: serde_json::Value::Object(Default::default()),
        };
        let dispatcher = TestDispatcher::simple().with_job(job);
        let jobs = prepare_reply_deliveries(&dispatcher);
        assert_eq!(jobs.len(), 1);
    }
}
