use thiserror::Error;

/// Errors originating in `ccb-completion`.
#[derive(Debug, Error)]
pub enum CompletionError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("unknown completion tracker: {0}")]
    UnknownTracker(String),

    #[error("storage error: {0}")]
    Storage(#[from] ccb_storage::StorageError),

    #[error("provider core error: {0}")]
    ProviderCore(#[from] ccb_provider_core::error::ProviderCoreError),

    #[error("agent error: {0}")]
    Agents(#[from] ccb_agents::AgentError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CompletionError>;
