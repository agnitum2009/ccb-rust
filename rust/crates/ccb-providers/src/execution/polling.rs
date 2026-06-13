use ccb_completion::models::{CompletionDecision, CompletionItem};

use super::handle::ExecutionServiceHandle;
use super::models::ExecutionUpdate;
use super::persistence::persist_submission;
use super::reliability::{
    adapter_reliability_policy, apply_reliability_progress, timeout_poll_result,
};

pub fn poll_updates(handle: &mut ExecutionServiceHandle) -> Vec<ExecutionUpdate> {
    let mut updates = Vec::new();
    let now = (handle.clock)();
    let replayed_job_ids = drain_pending_replays(handle, &mut updates);

    let active_jobs: Vec<String> = handle
        .active
        .keys()
        .filter(|job_id| !replayed_job_ids.contains(*job_id))
        .cloned()
        .collect();

    for job_id in active_jobs {
        let Some(submission) = handle.active.get(&job_id).cloned() else {
            continue;
        };
        process_active_job(handle, &mut updates, &job_id, submission, &now);
    }

    updates
}

type PendingReplay = (Vec<CompletionItem>, Option<CompletionDecision>);

fn drain_pending_replays(
    handle: &mut ExecutionServiceHandle,
    updates: &mut Vec<ExecutionUpdate>,
) -> std::collections::HashSet<String> {
    let mut replayed = std::collections::HashSet::new();
    let entries: Vec<(String, PendingReplay)> = handle
        .pending_replays
        .iter()
        .map(|(k, v)| (k.clone(), (v.0.clone(), v.1.clone())))
        .collect();
    for (job_id, (items, decision)) in entries {
        updates.push(ExecutionUpdate {
            job_id: job_id.clone(),
            items,
            decision: decision.clone(),
        });
        replayed.insert(job_id.clone());
        if decision.as_ref().is_some_and(|d| d.terminal) && !handle.active.contains_key(&job_id) {
            continue;
        }
        handle.pending_replays.remove(&job_id);
    }
    replayed
}

fn process_active_job(
    handle: &mut ExecutionServiceHandle,
    updates: &mut Vec<ExecutionUpdate>,
    job_id: &str,
    submission: super::models::ProviderSubmission,
    now: &str,
) {
    let Some(adapter) = handle.registry.get(&submission.provider_key()) else {
        handle.active.remove(job_id);
        return;
    };

    let mut result = adapter.poll(&submission, now);
    if result.is_none() {
        if let Some(policy) = adapter_reliability_policy(adapter.as_ref()) {
            result = timeout_poll_result(&submission, now, policy);
        }
        if result.is_none() {
            return;
        }
    } else {
        result = result.map(|r| apply_reliability_progress(r, &submission, now));
        if result.as_ref().is_some_and(|r| r.decision.is_none()) {
            if let Some(policy) = adapter_reliability_policy(adapter.as_ref()) {
                if let Some(timeout_result) =
                    timeout_poll_result(&result.as_ref().unwrap().submission, now, policy)
                {
                    result = Some(timeout_result);
                }
            }
        }
    }

    let result = result.unwrap();
    handle
        .active
        .insert(job_id.to_string(), result.submission.clone());
    persist_submission(
        handle,
        job_id,
        result.decision.clone().filter(|d| d.terminal),
        &result.items,
        &[],
    );

    if !should_emit_update(&result) {
        return;
    }

    updates.push(ExecutionUpdate {
        job_id: job_id.to_string(),
        items: result.items.clone(),
        decision: result.decision.clone(),
    });

    if result.decision.as_ref().is_some_and(|d| d.terminal) {
        handle.active.remove(job_id);
        handle.runtime_contexts.remove(job_id);
    }
}

fn should_emit_update(result: &super::models::ProviderPollResult) -> bool {
    !result.items.is_empty() || result.decision.is_some()
}
