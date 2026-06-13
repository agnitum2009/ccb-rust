use ccb_completion::models::JobRecord;

use super::handle::ExecutionServiceHandle;
use super::models::{
    ExecutionRestoreResult, ExecutionUpdate, ProviderExecutionRegistry, ProviderRuntimeContext,
    ProviderSubmission,
};
use super::persistence::{acknowledge, acknowledge_item, persist_submission};
use super::polling::poll_updates;
use super::restore::restore_submission;
use super::snapshots::active_runtime_snapshots;
use super::state_store::ExecutionStateStore;

/// Service that manages provider execution lifecycle.
pub struct ExecutionService {
    handle: ExecutionServiceHandle,
}

impl ExecutionService {
    pub fn new(
        registry: ProviderExecutionRegistry,
        clock: impl Fn() -> String + Send + Sync + 'static,
        state_store: Option<ExecutionStateStore>,
    ) -> Self {
        Self {
            handle: ExecutionServiceHandle::new(registry, clock, state_store),
        }
    }

    pub fn start(
        &mut self,
        job: &JobRecord,
        runtime_context: Option<&ProviderRuntimeContext>,
    ) -> Option<ProviderSubmission> {
        let now = (self.handle.clock)();
        let adapter = self.handle.registry.get(&job.provider)?;
        let submission = adapter.start(job, runtime_context, &now);
        self.handle
            .active
            .insert(job.job_id.clone(), submission.clone());
        if let Some(ctx) = runtime_context {
            self.handle
                .runtime_contexts
                .insert(job.job_id.clone(), ctx.clone());
        }
        persist_submission(&mut self.handle, &job.job_id, None, &[], &[]);
        Some(submission)
    }

    pub fn cancel(&mut self, job_id: &str) {
        let submission = self.handle.active.remove(job_id);
        if let Some(sub) = &submission {
            self.interrupt_active_submission(sub);
        }
        self.handle.runtime_contexts.remove(job_id);
        self.handle.pending_replays.remove(job_id);
        if let Some(store) = self.handle.state_store.as_ref() {
            store.remove(job_id);
        }
    }

    pub fn finish(&mut self, job_id: &str) {
        self.handle.active.remove(job_id);
        self.handle.runtime_contexts.remove(job_id);
        self.handle.pending_replays.remove(job_id);
        if let Some(store) = self.handle.state_store.as_ref() {
            store.remove(job_id);
        }
    }

    pub fn acknowledge(&mut self, job_id: &str) {
        acknowledge(&mut self.handle, job_id);
    }

    pub fn acknowledge_item(&mut self, job_id: &str, event_seq: Option<u64>) {
        acknowledge_item(&mut self.handle, job_id, event_seq);
    }

    pub fn restore(
        &mut self,
        job: &JobRecord,
        runtime_context: Option<&ProviderRuntimeContext>,
    ) -> ExecutionRestoreResult {
        restore_submission(&mut self.handle, job, runtime_context)
    }

    pub fn poll(&mut self) -> Vec<ExecutionUpdate> {
        poll_updates(&mut self.handle)
    }

    pub fn active_runtime_snapshots(
        &self,
    ) -> Vec<std::collections::HashMap<String, serde_json::Value>> {
        active_runtime_snapshots(&self.handle)
    }

    fn interrupt_active_submission(&self, submission: &ProviderSubmission) {
        let backend = submission.runtime_state.get("backend");
        let pane_id = submission
            .runtime_state
            .get("pane_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if backend.is_none() || pane_id.is_empty() {
            return;
        }
        // Terminal interruption is provider-specific and will be wired later.
        let _ = pane_id;
    }
}
