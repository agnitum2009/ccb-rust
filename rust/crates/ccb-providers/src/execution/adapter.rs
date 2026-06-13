use std::collections::HashMap;

use ccb_completion::models::JobRecord;
use serde_json::Value;

use super::models::{
    PersistedExecutionState, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};

/// Trait for provider execution adapters.
/// Mirrors Python `provider_execution.base.ProviderExecutionAdapter`.
pub trait ExecutionAdapter: Send + Sync {
    fn provider(&self) -> &str;

    fn start(
        &self,
        job: &JobRecord,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission;

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult>;

    /// Export the runtime state of a submission for persistence.
    fn export_runtime_state(
        &self,
        submission: &ProviderSubmission,
    ) -> Option<HashMap<String, Value>> {
        Some(submission.runtime_state.clone())
    }

    /// Resume a submission from persisted state.
    fn resume(
        &self,
        _job: &JobRecord,
        submission: &ProviderSubmission,
        _context: Option<&ProviderRuntimeContext>,
        _persisted_state: &PersistedExecutionState,
        _now: &str,
    ) -> Option<ProviderSubmission> {
        Some(submission.clone())
    }
}

/// Boxed execution adapter for storage in registries.
pub type AdapterBox = Box<dyn ExecutionAdapter>;
