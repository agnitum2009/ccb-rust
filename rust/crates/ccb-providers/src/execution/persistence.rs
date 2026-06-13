use std::collections::HashMap;

use ccb_completion::models::{CompletionDecision, CompletionItem};
use serde_json::Value;

use super::handle::ExecutionServiceHandle;
use super::models::{PersistedExecutionState, ProviderSubmission};

/// Acknowledge all pending items for a job.
pub fn acknowledge(handle: &mut ExecutionServiceHandle, job_id: &str) {
    handle.pending_replays.remove(job_id);
    let Some(store) = handle.state_store.as_ref() else {
        return;
    };
    let Some(persisted) = store.load(job_id) else {
        return;
    };
    let _ = store.save(&PersistedExecutionState {
        pending_items: Vec::new(),
        applied_event_seqs: Vec::new(),
        persisted_at: (handle.clock)(),
        ..persisted
    });
}

/// Acknowledge a single pending item by event sequence.
pub fn acknowledge_item(handle: &mut ExecutionServiceHandle, job_id: &str, event_seq: Option<u64>) {
    let Some(event_seq) = event_seq else {
        return;
    };
    let Some(store) = handle.state_store.as_ref() else {
        return;
    };
    let Some(persisted) = store.load(job_id) else {
        return;
    };
    if persisted.pending_items.is_empty() {
        return;
    }
    let mut applied: std::collections::BTreeSet<_> =
        persisted.applied_event_seqs.iter().copied().collect();
    applied.insert(event_seq);
    let _ = store.save(&PersistedExecutionState {
        applied_event_seqs: applied.into_iter().collect(),
        persisted_at: (handle.clock)(),
        ..persisted
    });
}

/// Persist the current active submission for a job.
pub fn persist_submission(
    handle: &mut ExecutionServiceHandle,
    job_id: &str,
    pending_decision: Option<CompletionDecision>,
    pending_items: &[CompletionItem],
    applied_event_seqs: &[u64],
) {
    let Some(store) = handle.state_store.as_ref() else {
        return;
    };
    let Some(submission) = handle.active.get(job_id).cloned() else {
        return;
    };
    let adapter = handle.registry.get(&submission.provider_key());
    let capability = execution_restore_capability(adapter, &submission.provider);
    let runtime_state = adapter
        .and_then(|a| a.export_runtime_state(&submission))
        .map(|exported| with_reliability_state(exported, &submission.runtime_state))
        .unwrap_or_else(|| with_reliability_state(HashMap::new(), &submission.runtime_state));
    let mut diagnostics = submission
        .diagnostics
        .clone()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Value::Object(ref mut obj) = diagnostics {
        for (k, v) in capability.as_object().unwrap_or(&serde_json::Map::new()) {
            obj.insert(k.clone(), v.clone());
        }
    }
    let persisted = PersistedExecutionState {
        schema_version: super::models::EXECUTION_STATE_SCHEMA_VERSION,
        record_type: "execution_state".to_string(),
        submission: ProviderSubmission {
            diagnostics: Some(diagnostics),
            runtime_state,
            ..submission
        },
        runtime_context: handle.runtime_contexts.get(job_id).cloned(),
        resume_capable: capability
            .get("resume_supported")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        persisted_at: (handle.clock)(),
        pending_decision,
        pending_items: pending_items.to_vec(),
        applied_event_seqs: applied_event_seqs.to_vec(),
    };
    let _ = store.save(&persisted);
}

/// Filter pending items to remove those already applied.
pub fn filter_pending_items(persisted: &PersistedExecutionState) -> Vec<CompletionItem> {
    if persisted.pending_items.is_empty() || persisted.applied_event_seqs.is_empty() {
        return persisted.pending_items.clone();
    }
    let applied: std::collections::BTreeSet<_> =
        persisted.applied_event_seqs.iter().copied().collect();
    persisted
        .pending_items
        .iter()
        .filter(|item| {
            item.cursor
                .event_seq
                .map(|seq| !applied.contains(&seq))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

fn with_reliability_state(
    runtime_state: HashMap<String, Value>,
    source_state: &HashMap<String, Value>,
) -> HashMap<String, Value> {
    let reliability_state: HashMap<_, _> = source_state
        .iter()
        .filter(|(k, _)| k.starts_with("reliability_"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if reliability_state.is_empty() {
        return runtime_state;
    }
    let mut out = runtime_state;
    out.extend(reliability_state);
    out
}

fn execution_restore_capability(
    adapter: Option<&super::adapter::AdapterBox>,
    provider: &str,
) -> Value {
    let Some(adapter) = adapter else {
        return serde_json::json!({
            "resume_supported": false,
            "restore_mode": "resubmit_required",
            "restore_reason": "adapter_missing",
            "restore_detail": format!("provider {provider} has no registered execution adapter"),
        });
    };
    let supports_export = adapter
        .export_runtime_state(&ProviderSubmission {
            job_id: String::new(),
            agent_name: String::new(),
            provider: provider.to_string(),
            accepted_at: String::new(),
            ready_at: String::new(),
            source_kind: ccb_completion::models::CompletionSourceKind::ProtocolEventStream,
            reply: String::new(),
            status: ccb_completion::models::CompletionStatus::Incomplete,
            reason: String::new(),
            confidence: ccb_completion::models::CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state: HashMap::new(),
        })
        .is_some();
    let supports_resume = true; // Trait provides default resume.
    let resume_supported = supports_export && supports_resume;
    let restore_mode = if resume_supported {
        "provider_resume"
    } else {
        "resubmit_required"
    };
    let restore_reason = if resume_supported {
        None
    } else {
        Some("provider_resume_unsupported")
    };
    let restore_detail = if resume_supported {
        "provider execution can be resumed after ccbd restart"
    } else {
        "provider execution cannot be resumed after ccbd restart and requires resubmission"
    };
    serde_json::json!({
        "resume_supported": resume_supported,
        "restore_mode": restore_mode,
        "restore_reason": restore_reason,
        "restore_detail": restore_detail,
    })
}
