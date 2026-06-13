use ccb_completion::models::{CompletionDecision, CompletionItem, JobRecord};

use super::handle::ExecutionServiceHandle;
use super::models::{
    ExecutionRestoreResult, PersistedExecutionState, ProviderRuntimeContext, ProviderSubmission,
};
use super::persistence::{filter_pending_items, persist_submission};

pub fn restore_submission(
    handle: &mut ExecutionServiceHandle,
    job: &JobRecord,
    runtime_context: Option<&ProviderRuntimeContext>,
) -> ExecutionRestoreResult {
    if handle.active.contains_key(&job.job_id) {
        return result(job, "restored", "already_active", true, 0, None);
    }
    if handle.state_store.is_none() {
        return result(job, "missing", "state_store_disabled", false, 0, None);
    }

    let adapter = handle.registry.get(&job.provider);
    if adapter.is_none() {
        abandon_restore(handle, job, "adapter_missing", false, 0);
        return result(job, "abandoned", "adapter_missing", false, 0, None);
    }

    let persisted = match handle.state_store.as_ref().unwrap().load(&job.job_id) {
        Some(p) => p,
        None => return result(job, "missing", "state_missing", false, 0, None),
    };

    if persisted.provider() != job.provider {
        abandon_restore(
            handle,
            job,
            "provider_mismatch",
            persisted.resume_capable,
            persisted.pending_items.len(),
        );
        return result(
            job,
            "abandoned",
            "provider_mismatch",
            persisted.resume_capable,
            persisted.pending_items.len(),
            persisted.pending_decision,
        );
    }

    let pending_items = recover_pending_items(handle, &job.job_id, &persisted);

    if persisted.pending_decision.is_some() && pending_items.is_empty() {
        return result(
            job,
            "terminal_pending",
            "terminal_decision_recovered",
            persisted.resume_capable,
            0,
            persisted.pending_decision,
        );
    }

    let restored_context = runtime_context
        .cloned()
        .or(persisted.runtime_context.clone());
    let submission = match resume_submission(handle, job, &persisted, restored_context.as_ref()) {
        Some(s) => s,
        None => {
            abandon_restore(
                handle,
                job,
                "provider_resume_rejected",
                persisted.resume_capable,
                pending_items.len(),
            );
            return result(
                job,
                "abandoned",
                "provider_resume_rejected",
                persisted.resume_capable,
                pending_items.len(),
                None,
            );
        }
    };

    handle.active.insert(job.job_id.clone(), submission.clone());
    handle
        .runtime_contexts
        .insert(job.job_id.clone(), restored_context.unwrap_or_default());
    persist_submission(
        handle,
        &job.job_id,
        persisted.pending_decision.clone(),
        &pending_items,
        &persisted.applied_event_seqs,
    );

    result(
        job,
        if pending_items.is_empty() {
            "restored"
        } else {
            "replay_pending"
        },
        if pending_items.is_empty() {
            "provider_resumed"
        } else {
            "pending_items_recovered"
        },
        true,
        pending_items.len(),
        None,
    )
}

fn recover_pending_items(
    handle: &mut ExecutionServiceHandle,
    job_id: &str,
    persisted: &PersistedExecutionState,
) -> Vec<CompletionItem> {
    let pending = filter_pending_items(persisted);
    if !pending.is_empty() {
        handle.pending_replays.insert(
            job_id.to_string(),
            (pending.clone(), persisted.pending_decision.clone()),
        );
    }
    pending
}

fn resume_submission(
    handle: &ExecutionServiceHandle,
    job: &JobRecord,
    persisted: &PersistedExecutionState,
    context: Option<&ProviderRuntimeContext>,
) -> Option<ProviderSubmission> {
    if !persisted.resume_capable {
        return None;
    }
    let adapter = handle.registry.get(&job.provider)?;
    adapter.resume(
        job,
        &persisted.submission,
        context,
        persisted,
        &(handle.clock)(),
    )
}

fn abandon_restore(
    handle: &mut ExecutionServiceHandle,
    job: &JobRecord,
    reason: &str,
    resume_capable: bool,
    pending_items_count: usize,
) -> ExecutionRestoreResult {
    if let Some(store) = handle.state_store.as_ref() {
        store.remove(&job.job_id);
    }
    result(
        job,
        "abandoned",
        reason,
        resume_capable,
        pending_items_count,
        None,
    )
}

fn result(
    job: &JobRecord,
    status: &str,
    reason: &str,
    resume_capable: bool,
    pending_items_count: usize,
    decision: Option<CompletionDecision>,
) -> ExecutionRestoreResult {
    ExecutionRestoreResult {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: job.provider.clone(),
        status: status.to_string(),
        reason: reason.to_string(),
        resume_capable,
        pending_items_count,
        decision,
    }
}
