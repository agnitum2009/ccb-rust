use std::collections::HashMap;

use ccb_completion::models::{CompletionDecision, CompletionItem};

use super::models::{ProviderExecutionRegistry, ProviderRuntimeContext, ProviderSubmission};
use super::state_store::ExecutionStateStore;

/// Mutable handle passed to execution helper functions.
pub struct ExecutionServiceHandle {
    pub registry: ProviderExecutionRegistry,
    pub clock: Box<dyn Fn() -> String + Send + Sync>,
    pub state_store: Option<ExecutionStateStore>,
    pub active: HashMap<String, ProviderSubmission>,
    pub runtime_contexts: HashMap<String, ProviderRuntimeContext>,
    pub pending_replays: HashMap<String, (Vec<CompletionItem>, Option<CompletionDecision>)>,
}

impl ExecutionServiceHandle {
    pub fn new(
        registry: ProviderExecutionRegistry,
        clock: impl Fn() -> String + Send + Sync + 'static,
        state_store: Option<ExecutionStateStore>,
    ) -> Self {
        Self {
            registry,
            clock: Box::new(clock),
            state_store,
            active: HashMap::new(),
            runtime_contexts: HashMap::new(),
            pending_replays: HashMap::new(),
        }
    }
}
