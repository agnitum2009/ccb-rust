use std::collections::HashMap;

use ccb_completion::models::{
    CompletionConfidence, CompletionDecision, CompletionItem, CompletionSourceKind,
    CompletionStatus, JobRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Schema version for persisted execution state records.
pub const EXECUTION_STATE_SCHEMA_VERSION: u32 = 3;

/// Runtime context for a provider execution.
/// Mirrors Python `provider_execution.base.ProviderRuntimeContext`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderRuntimeContext {
    pub agent_name: String,
    pub workspace_path: Option<String>,
    pub backend_type: Option<String>,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    #[serde(default)]
    pub runtime_pid: Option<u32>,
    #[serde(default)]
    pub runtime_health: Option<String>,
    #[serde(default)]
    pub runtime_binding_source: Option<String>,
}

/// A submission to a provider for execution.
/// Mirrors Python `provider_execution.base.ProviderSubmission`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSubmission {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    pub accepted_at: String,
    pub ready_at: String,
    pub source_kind: CompletionSourceKind,
    pub reply: String,
    pub status: CompletionStatus,
    #[serde(default = "default_reason")]
    pub reason: String,
    pub confidence: CompletionConfidence,
    #[serde(default)]
    pub diagnostics: Option<Value>,
    #[serde(default)]
    pub runtime_state: HashMap<String, Value>,
}

fn default_reason() -> String {
    "in_progress".to_string()
}

impl ProviderSubmission {
    pub fn new(
        job: &JobRecord,
        provider: impl Into<String>,
        now: impl Into<String>,
        source_kind: CompletionSourceKind,
    ) -> Self {
        let now = now.into();
        Self {
            job_id: job.job_id.clone(),
            agent_name: job.agent_name.clone(),
            provider: provider.into(),
            accepted_at: now.clone(),
            ready_at: now,
            source_kind,
            reply: String::new(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".to_string(),
            confidence: CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state: HashMap::new(),
        }
    }

    /// Convenience accessor for the provider name normalized to lowercase.
    pub fn provider_key(&self) -> String {
        self.provider.trim().to_lowercase()
    }

    /// True when the submission is in a terminal completion status.
    /// Note: ccb-completion treats `Incomplete` as terminal; execution treats it as in-progress.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            CompletionStatus::Completed | CompletionStatus::Cancelled | CompletionStatus::Failed
        )
    }
}

/// Result of polling a provider submission.
/// Mirrors Python `provider_execution.base.ProviderPollResult`.
#[derive(Debug, Clone)]
pub struct ProviderPollResult {
    pub submission: ProviderSubmission,
    pub items: Vec<CompletionItem>,
    pub decision: Option<CompletionDecision>,
}

impl ProviderPollResult {
    /// Build a new poll result, validating that any decision is terminal.
    pub fn new(
        submission: ProviderSubmission,
        items: Vec<CompletionItem>,
        decision: Option<CompletionDecision>,
    ) -> Self {
        if let Some(d) = &decision {
            assert!(d.terminal, "provider poll decisions must be terminal");
        }
        Self {
            submission,
            items,
            decision,
        }
    }
}

/// An update emitted by the execution service after a poll cycle.
#[derive(Debug, Clone)]
pub struct ExecutionUpdate {
    pub job_id: String,
    pub items: Vec<CompletionItem>,
    pub decision: Option<CompletionDecision>,
}

/// Result of restoring a persisted execution.
#[derive(Debug, Clone)]
pub struct ExecutionRestoreResult {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    pub status: String,
    pub reason: String,
    pub resume_capable: bool,
    pub pending_items_count: usize,
    pub decision: Option<CompletionDecision>,
}

impl ExecutionRestoreResult {
    pub fn restored(&self) -> bool {
        matches!(
            self.status.as_str(),
            "restored" | "terminal_pending" | "replay_pending"
        )
    }
}

/// Persisted execution state record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedExecutionState {
    pub schema_version: u32,
    pub record_type: String,
    pub submission: ProviderSubmission,
    pub runtime_context: Option<ProviderRuntimeContext>,
    pub resume_capable: bool,
    pub persisted_at: String,
    #[serde(default)]
    pub pending_decision: Option<CompletionDecision>,
    #[serde(default)]
    pub pending_items: Vec<CompletionItem>,
    #[serde(default)]
    pub applied_event_seqs: Vec<u64>,
}

impl PersistedExecutionState {
    pub fn new(
        submission: ProviderSubmission,
        runtime_context: Option<ProviderRuntimeContext>,
        resume_capable: bool,
        persisted_at: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: EXECUTION_STATE_SCHEMA_VERSION,
            record_type: "execution_state".to_string(),
            submission,
            runtime_context,
            resume_capable,
            persisted_at: persisted_at.into(),
            pending_decision: None,
            pending_items: Vec::new(),
            applied_event_seqs: Vec::new(),
        }
    }

    pub fn job_id(&self) -> &str {
        &self.submission.job_id
    }

    pub fn provider(&self) -> &str {
        &self.submission.provider
    }
}

/// Registry of execution adapters keyed by provider name.
#[derive(Default)]
pub struct ProviderExecutionRegistry {
    adapters: HashMap<String, super::adapter::AdapterBox>,
}

impl ProviderExecutionRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, adapter: super::adapter::AdapterBox) {
        let provider = adapter.provider().trim().to_lowercase();
        if self.adapters.contains_key(&provider) {
            panic!("duplicate execution adapter: {provider}");
        }
        self.adapters.insert(provider, adapter);
    }

    pub fn get(&self, provider: &str) -> Option<&super::adapter::AdapterBox> {
        let key = provider.trim().to_lowercase();
        self.adapters.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submission_new() {
        let job = JobRecord::new("j1", "agent1", "claude");
        let sub = ProviderSubmission::new(
            &job,
            "claude",
            "2025-01-01T00:00:00Z",
            CompletionSourceKind::ProtocolEventStream,
        );
        assert_eq!(sub.job_id, "j1");
        assert_eq!(sub.provider_key(), "claude");
        assert!(!sub.is_terminal());
    }

    #[test]
    fn test_restore_result_restored() {
        let r = ExecutionRestoreResult {
            job_id: "j1".into(),
            agent_name: "agent1".into(),
            provider: "claude".into(),
            status: "restored".into(),
            reason: "provider_resumed".into(),
            resume_capable: true,
            pending_items_count: 0,
            decision: None,
        };
        assert!(r.restored());
    }
}
