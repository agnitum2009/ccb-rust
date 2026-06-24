use std::io;

/// Errors originating in `ccbr-provider-core`.
#[derive(Debug, thiserror::Error)]
pub enum ProviderCoreError {
    #[error("duplicate provider manifest: {0}")]
    DuplicateManifest(String),

    #[error("duplicate provider backend: {0}")]
    DuplicateBackend(String),

    #[error("unknown provider: {0}")]
    UnknownProvider(String),

    #[error("provider cannot be empty")]
    EmptyProvider,

    #[error("runtime profiles cannot be empty")]
    EmptyRuntimeProfiles,

    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("model cannot be empty")]
    EmptyModel,

    #[error("command template must contain exactly one {{command}} placeholder")]
    InvalidCommandTemplate,

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("storage error: {0}")]
    Storage(#[from] ccbr_storage::StorageError),
}

/// Convenience result type for provider-core operations.
pub type Result<T> = std::result::Result<T, ProviderCoreError>;
