//! Adapters between daemon API models and the `ccb-completion` crate.

use ccb_completion::models::{JobRecord as CompletionJobRecord, JobRequest, TargetKind};

/// Convert a daemon `JobRecord` into the `ccb-completion` job record shape.
pub fn to_completion_job_record(
    job: &crate::models::api_models::records::JobRecord,
) -> CompletionJobRecord {
    CompletionJobRecord {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: job.provider.clone(),
        target_kind: TargetKind::Agent,
        request: JobRequest {
            body: job.request.body.clone(),
            message_type: Some(job.request.message_type.clone()),
        },
        provider_options: serde_json::Map::new(),
        workspace_path: job.workspace_path.clone(),
        provider_instance: None,
    }
}
