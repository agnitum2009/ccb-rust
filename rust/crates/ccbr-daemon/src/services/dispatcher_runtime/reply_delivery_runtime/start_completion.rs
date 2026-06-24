use crate::models::api_models::records::JobRecord;
use crate::services::dispatcher::JobDispatcher;
use crate::services::dispatcher_runtime::reply_delivery_runtime::decisions::{
    reply_delivery_completed_decision, reply_delivery_failed_decision,
};
use ccbr_completion::models::CompletionDecision;
use ccbr_providers::execution::models::ProviderSubmission;

/// Dispatcher operations needed by reply-delivery start-completion.
pub trait ReplyDeliveryDispatcher {
    /// Mark the job as complete with the given terminal decision.
    fn complete(&mut self, job_id: &str, decision: CompletionDecision) -> Option<&JobRecord>;
}

impl ReplyDeliveryDispatcher for JobDispatcher {
    fn complete(&mut self, job_id: &str, decision: CompletionDecision) -> Option<&JobRecord> {
        let status = crate::app::map_completion_status(decision.status);
        let decision_record = crate::app::decision_to_record(&decision);
        self.update_job_status(job_id, status, Some(decision_record));
        self.get(job_id)
    }
}

/// Complete a reply-delivery job after the provider has started.
///
/// Mirrors Python `complete_reply_delivery_after_start`.
///
/// - If the submission mode is `"error"` or `"passive"`, mark the job failed.
/// - If `reply_delivery_complete_on_dispatch` is true, return the job as-is
///   without marking it complete (the provider will finish delivery itself).
/// - Otherwise mark the job completed.
pub fn complete_reply_delivery_after_start<'a, D: ReplyDeliveryDispatcher>(
    dispatcher: &'a mut D,
    job: &'a JobRecord,
    started_at: &'a str,
    submission: Option<&'a ProviderSubmission>,
) -> Option<&'a JobRecord> {
    let mode = submission
        .and_then(|s| s.runtime_state.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if mode == "error" || mode == "passive" {
        let decision =
            reply_delivery_failed_decision(started_at, "reply_delivery_not_supported", None);
        return dispatcher.complete(&job.job_id, decision);
    }

    let complete_on_dispatch = submission
        .and_then(|s| s.runtime_state.get("reply_delivery_complete_on_dispatch"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if complete_on_dispatch {
        return Some(job);
    }

    let provider_turn_ref = submission
        .and_then(|s| s.runtime_state.get("provider_turn_ref"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            job.terminal_decision
                .as_ref()
                .and_then(|d| d.get("provider_turn_ref"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    let decision = reply_delivery_completed_decision(started_at, provider_turn_ref.as_deref());
    dispatcher.complete(&job.job_id, decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api_models::common::{DeliveryScope, JobStatus};
    use crate::models::api_models::messages::MessageEnvelope;
    use crate::services::dispatcher::JobDispatcher;
    use ccbr_completion::models::{CompletionConfidence, CompletionStatus};
    use ccbr_providers::execution::models::ProviderSubmission;
    use std::collections::HashMap;

    fn test_envelope(to_agent: &str) -> MessageEnvelope {
        MessageEnvelope {
            project_id: "p1".into(),
            to_agent: to_agent.into(),
            from_actor: "user".into(),
            body: "hello".into(),
            task_id: None,
            reply_to: None,
            message_type: "ask".into(),
            delivery_scope: DeliveryScope::Single,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        }
    }

    fn make_submission_with_flag(flag: bool) -> ProviderSubmission {
        let mut runtime_state = HashMap::new();
        runtime_state.insert(
            "reply_delivery_complete_on_dispatch".into(),
            serde_json::Value::Bool(flag),
        );
        ProviderSubmission {
            job_id: "job_1".into(),
            agent_name: "claude".into(),
            provider: "claude".into(),
            accepted_at: "2025-01-01T00:00:00Z".into(),
            ready_at: "2025-01-01T00:00:00Z".into(),
            source_kind: ccbr_completion::models::CompletionSourceKind::ProtocolEventStream,
            reply: String::new(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".into(),
            confidence: CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state,
        }
    }

    #[test]
    fn complete_reply_delivery_defer_when_provider_requests_dispatch_completion() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude"), "claude", None);
        let job_id = &receipt.jobs[0].job_id;
        dispatcher.tick();
        let job = dispatcher.get(job_id).unwrap().clone();

        let submission = make_submission_with_flag(true);
        let returned = complete_reply_delivery_after_start(
            &mut dispatcher,
            &job,
            "2025-01-01T00:00:01Z",
            Some(&submission),
        );

        assert!(returned.is_some());
        assert_eq!(returned.unwrap().job_id, job.job_id);
        assert_eq!(dispatcher.get(job_id).unwrap().status, JobStatus::Running);
        assert!(dispatcher.get(job_id).unwrap().terminal_decision.is_none());
    }

    #[test]
    fn complete_reply_delivery_completes_active_mode() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude"), "claude", None);
        let job_id = &receipt.jobs[0].job_id;
        dispatcher.tick();
        let job = dispatcher.get(job_id).unwrap().clone();

        let submission = make_submission_with_flag(false);
        let returned = complete_reply_delivery_after_start(
            &mut dispatcher,
            &job,
            "2025-01-01T00:00:01Z",
            Some(&submission),
        );

        assert!(returned.is_some());
        let updated = dispatcher.get(job_id).unwrap();
        assert_eq!(updated.status, JobStatus::Completed);
        assert!(updated.terminal_decision.is_some());
    }

    #[test]
    fn complete_reply_delivery_fails_for_error_mode() {
        let mut dispatcher = JobDispatcher::new(vec!["claude".into()]);
        let receipt = dispatcher.submit(&test_envelope("claude"), "claude", None);
        let job_id = &receipt.jobs[0].job_id;
        dispatcher.tick();
        let job = dispatcher.get(job_id).unwrap().clone();

        let mut submission = make_submission_with_flag(false);
        submission
            .runtime_state
            .insert("mode".into(), serde_json::Value::String("error".into()));
        let returned = complete_reply_delivery_after_start(
            &mut dispatcher,
            &job,
            "2025-01-01T00:00:01Z",
            Some(&submission),
        );

        assert!(returned.is_some());
        let updated = dispatcher.get(job_id).unwrap();
        assert_eq!(updated.status, JobStatus::Failed);
    }
}
