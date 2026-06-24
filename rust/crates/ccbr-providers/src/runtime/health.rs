use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROVIDER_HEALTH_SCHEMA_VERSION: u32 = 1;

/// Progress state for a provider health snapshot.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressState {
    NotStarted,
    Submitted,
    Accepted,
    ActivelyRunning,
    QuietWait,
    OutputAdvancing,
    Stalled,
    RuntimeLost,
    SessionLost,
    #[default]
    Unknown,
}

/// Completion state for a provider health snapshot.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCompletionState {
    NotComplete,
    TerminalComplete,
    TerminalIncomplete,
    TerminalFailed,
    TerminalCancelled,
    #[default]
    Indeterminate,
}

/// A point-in-time health snapshot for a provider execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthSnapshot {
    pub schema_version: u32,
    pub record_type: String,
    pub job_id: String,
    pub provider: String,
    pub agent_name: String,
    pub runtime_alive: bool,
    pub session_reachable: Option<bool>,
    pub progress_state: ProgressState,
    pub completion_state: ProviderCompletionState,
    pub last_progress_at: Option<String>,
    pub observed_at: String,
    pub degraded_reason: Option<String>,
    pub diagnostics: HashMap<String, Value>,
}

impl ProviderHealthSnapshot {
    pub fn new(
        job_id: impl Into<String>,
        provider: impl Into<String>,
        agent_name: impl Into<String>,
        observed_at: impl Into<String>,
    ) -> Self {
        let job_id = job_id.into();
        let provider = provider.into();
        let agent_name = agent_name.into();
        assert!(!job_id.is_empty(), "job_id cannot be empty");
        assert!(!provider.is_empty(), "provider cannot be empty");
        assert!(!agent_name.is_empty(), "agent_name cannot be empty");
        Self {
            schema_version: PROVIDER_HEALTH_SCHEMA_VERSION,
            record_type: "provider_health_snapshot".to_string(),
            job_id,
            provider,
            agent_name,
            runtime_alive: false,
            session_reachable: None,
            progress_state: ProgressState::default(),
            completion_state: ProviderCompletionState::default(),
            last_progress_at: None,
            observed_at: observed_at.into(),
            degraded_reason: None,
            diagnostics: HashMap::new(),
        }
    }

    pub fn with_runtime_alive(mut self, alive: bool) -> Self {
        self.runtime_alive = alive;
        self
    }

    pub fn with_session_reachable(mut self, reachable: Option<bool>) -> Self {
        self.session_reachable = reachable;
        self
    }

    pub fn with_progress_state(mut self, state: ProgressState) -> Self {
        self.progress_state = state;
        self
    }

    pub fn with_completion_state(mut self, state: ProviderCompletionState) -> Self {
        self.completion_state = state;
        self
    }

    pub fn with_last_progress_at(mut self, at: impl Into<String>) -> Self {
        self.last_progress_at = Some(at.into());
        self
    }

    pub fn with_degraded_reason(mut self, reason: impl Into<String>) -> Self {
        self.degraded_reason = Some(reason.into());
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: HashMap<String, Value>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    pub fn to_record(&self) -> Value {
        serde_json::json!({
            "schema_version": self.schema_version,
            "record_type": self.record_type,
            "job_id": self.job_id,
            "provider": self.provider,
            "agent_name": self.agent_name,
            "runtime_alive": self.runtime_alive,
            "session_reachable": self.session_reachable,
            "progress_state": self.progress_state,
            "completion_state": self.completion_state,
            "last_progress_at": self.last_progress_at,
            "observed_at": self.observed_at,
            "degraded_reason": self.degraded_reason,
            "diagnostics": self.diagnostics,
        })
    }
}
