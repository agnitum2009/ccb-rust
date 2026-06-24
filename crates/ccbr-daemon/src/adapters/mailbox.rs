/// Convert daemon API models to `ccbr_mailbox` models.
///
/// The two crates define similarly-named types (MessageEnvelope, JobRecord,
/// CompletionDecision) but they are independent. Keep conversions explicit and
/// in one place so neither crate has to depend on the other.
use ccbr_completion::models::CompletionStatus;

/// Convert a daemon message envelope to a mailbox envelope.
pub fn to_mailbox_envelope(
    envelope: &crate::models::api_models::messages::MessageEnvelope,
) -> ccbr_jobs::models::MessageEnvelope {
    ccbr_jobs::models::MessageEnvelope {
        project_id: envelope.project_id.clone(),
        to_agent: envelope.to_agent.clone(),
        from_actor: envelope.from_actor.clone(),
        body: envelope.body.clone(),
        task_id: envelope.task_id.clone(),
        reply_to: envelope.reply_to.clone(),
        message_type: envelope.message_type.clone(),
        delivery_scope: to_mailbox_delivery_scope(envelope.delivery_scope),
        silence_on_success: envelope.silence_on_success,
        route_options: envelope.route_options.clone(),
        body_artifact: envelope.body_artifact.clone(),
    }
}

fn to_mailbox_delivery_scope(
    scope: crate::models::api_models::common::DeliveryScope,
) -> ccbr_jobs::models::DeliveryScope {
    match scope {
        crate::models::api_models::common::DeliveryScope::Single => {
            ccbr_jobs::models::DeliveryScope::Agent
        }
        crate::models::api_models::common::DeliveryScope::Broadcast => {
            ccbr_jobs::models::DeliveryScope::Broadcast
        }
    }
}

/// Convert a daemon job record to a mailbox job record.
pub fn to_mailbox_job_record(
    job: &crate::models::api_models::records::JobRecord,
) -> ccbr_jobs::models::JobRecord {
    ccbr_jobs::models::JobRecord {
        job_id: job.job_id.clone(),
        submission_id: job.submission_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: job.provider.clone(),
        request: to_mailbox_envelope(&job.request),
        status: to_mailbox_job_status(job.status),
        terminal_decision: job.terminal_decision.clone(),
        cancel_requested_at: job.cancel_requested_at.clone(),
        created_at: job.created_at.clone(),
        updated_at: job.updated_at.clone(),
        workspace_path: job.workspace_path.clone(),
        target_kind: to_mailbox_target_kind(job.target_kind),
        target_name: job.target_name.clone(),
        provider_instance: None,
        provider_options: serde_json::Value::Object(serde_json::Map::new()),
    }
}

fn to_mailbox_job_status(
    status: crate::models::api_models::common::JobStatus,
) -> ccbr_jobs::models::JobStatus {
    use crate::models::api_models::common::JobStatus as DaemonJobStatus;
    match status {
        DaemonJobStatus::Accepted => ccbr_jobs::models::JobStatus::Accepted,
        DaemonJobStatus::Queued | DaemonJobStatus::Running => ccbr_jobs::models::JobStatus::Running,
        DaemonJobStatus::Completed => ccbr_jobs::models::JobStatus::Completed,
        DaemonJobStatus::Cancelled => ccbr_jobs::models::JobStatus::Cancelled,
        DaemonJobStatus::Failed => ccbr_jobs::models::JobStatus::Failed,
        DaemonJobStatus::Incomplete => ccbr_jobs::models::JobStatus::Incomplete,
    }
}

fn to_mailbox_target_kind(
    kind: crate::models::api_models::common::TargetKind,
) -> ccbr_jobs::models::TargetKind {
    match kind {
        crate::models::api_models::common::TargetKind::Agent => {
            ccbr_jobs::models::TargetKind::Agent
        }
    }
}

/// Convert a completion-layer decision to the mailbox decision shape.
pub fn to_mailbox_completion_decision(
    decision: &ccbr_completion::models::CompletionDecision,
) -> ccbr_mailbox::facade_recording::CompletionDecision {
    ccbr_mailbox::facade_recording::CompletionDecision {
        terminal: decision.terminal,
        status: completion_status_to_mailbox(decision.status),
        reason: decision.reason.clone(),
        reply: decision.reply.clone(),
        provider_turn_ref: decision.provider_turn_ref.clone(),
        diagnostics: serde_json::Value::Object(decision.diagnostics.clone()),
    }
}

fn completion_status_to_mailbox(status: CompletionStatus) -> ccbr_jobs::models::JobStatus {
    match status {
        CompletionStatus::Completed => ccbr_jobs::models::JobStatus::Completed,
        CompletionStatus::Cancelled => ccbr_jobs::models::JobStatus::Cancelled,
        CompletionStatus::Failed => ccbr_jobs::models::JobStatus::Failed,
        CompletionStatus::Incomplete => ccbr_jobs::models::JobStatus::Incomplete,
    }
}
