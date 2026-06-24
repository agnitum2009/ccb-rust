use serde::{Deserialize, Serialize};

/// Source kind for a completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionSourceKind {
    Direct,
    Delegate,
    Retry,
}

/// Completion status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CompletionStatus {
    #[default]
    Incomplete,
    Complete,
    Failed,
    TimedOut,
}

/// Confidence level of a completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CompletionConfidence {
    #[default]
    Observed,
    Declared,
    Inferred,
}

/// Runtime context for a provider execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSubmission {
    pub job_id: String,
    pub agent_name: String,
    pub provider: String,
    pub accepted_at: String,
    pub ready_at: String,
    pub source_kind: CompletionSourceKind,
    pub reply: String,
    #[serde(default)]
    pub status: CompletionStatus,
    #[serde(default = "default_reason")]
    pub reason: String,
    #[serde(default)]
    pub confidence: CompletionConfidence,
    #[serde(default)]
    pub diagnostics: Option<serde_json::Value>,
    #[serde(default)]
    pub runtime_state: std::collections::HashMap<String, serde_json::Value>,
}

fn default_reason() -> String {
    "in_progress".to_string()
}

/// Result of polling a provider submission.
#[derive(Debug, Clone)]
pub struct ProviderPollResult {
    pub submission: ProviderSubmission,
    pub items: Vec<CompletionItem>,
    pub decision: Option<CompletionDecision>,
}

/// A completion item from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub reply: String,
    pub source_kind: CompletionSourceKind,
    pub status: CompletionStatus,
    pub confidence: CompletionConfidence,
}

/// A terminal completion decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionDecision {
    pub terminal: bool,
    pub status: CompletionStatus,
    pub reason: String,
}

/// Trait for provider execution adapters.
/// In Python this was a Protocol class; in Rust it's a trait.
pub trait ExecutionAdapter: Send + Sync {
    fn provider(&self) -> &str;

    fn start(
        &self,
        job_id: &str,
        agent_name: &str,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission;

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_status_default() {
        assert_eq!(CompletionStatus::default(), CompletionStatus::Incomplete);
    }

    #[test]
    fn test_provider_submission_serde() {
        let sub = ProviderSubmission {
            job_id: "j1".into(),
            agent_name: "a1".into(),
            provider: "claude".into(),
            accepted_at: "2025-01-01T00:00:00Z".into(),
            ready_at: "2025-01-01T00:00:01Z".into(),
            source_kind: CompletionSourceKind::Direct,
            reply: String::new(),
            status: CompletionStatus::default(),
            reason: "in_progress".into(),
            confidence: CompletionConfidence::default(),
            diagnostics: None,
            runtime_state: Default::default(),
        };
        let json = serde_json::to_string(&sub).unwrap();
        let deserialized: ProviderSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.job_id, "j1");
    }
}
