pub mod api_shortcuts;
pub mod codex_home_config;
pub mod materializer;
pub mod models;

pub use api_shortcuts::*;
pub use codex_home_config::*;
pub use materializer::*;
pub use models::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfilesError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml error: {0}")]
    Toml(String),

    #[error("storage error: {0}")]
    Storage(#[from] ccb_storage::StorageError),

    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
}

impl From<toml::ser::Error> for ProfilesError {
    fn from(err: toml::ser::Error) -> Self {
        ProfilesError::Toml(err.to_string())
    }
}

impl From<toml::de::Error> for ProfilesError {
    fn from(err: toml::de::Error) -> Self {
        ProfilesError::Toml(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ProfilesError>;
