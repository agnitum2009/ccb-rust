use thiserror::Error;

/// Errors originating in `ccbr-completion`.
#[derive(Debug, Error)]
pub enum CompletionError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("unknown completion tracker: {0}")]
    UnknownTracker(String),

    #[error("storage error: {0}")]
    Storage(#[from] ccbr_storage::StorageError),

    #[error("provider core error: {0}")]
    ProviderCore(#[from] ccbr_provider_core::error::ProviderCoreError),

    #[error("agent error: {0}")]
    Agents(#[from] ccbr_agents::AgentError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CompletionError>;

/// Alias matching Python `CompletionValidationError`.
///
/// In Rust, validation failures are represented by the `Validation` variant of
/// `CompletionError`; this alias lets callers reference the Python-named type.
pub type CompletionValidationError = CompletionError;
