use serde_json::Value;

use crate::adapters::mailbox::to_mailbox_job_record;
use crate::app::CcbdApp;
use crate::models::api_models::common::JobStatus;

pub fn handle_cancel(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let job_id = payload
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if job_id.is_empty() {
        return Err("cancel requires job_id".into());
    }
    app.execution.cancel(job_id);
    let receipt = app.dispatcher.cancel(job_id);

    // Keep the mailbox layer consistent with the dispatcher: record a terminal
    // outcome when the job is actually cancelled.
    if receipt.status == JobStatus::Cancelled {
        if let Some(job) = app.dispatcher.get(job_id) {
            let mailbox_job = to_mailbox_job_record(job);
            let decision = ccbr_mailbox::facade_recording::CompletionDecision {
                terminal: true,
                status: ccbr_mailbox::models::JobStatus::Cancelled,
                reason: Some("cancelled".into()),
                reply: "".into(),
                provider_turn_ref: None,
                diagnostics: Value::Object(Default::default()),
            };
            let _ = app.mailbox.record_terminal(
                &mailbox_job,
                &decision,
                &receipt.cancelled_at,
                true,
                true,
            );
        }
    }

    Ok(receipt.to_record())
}
