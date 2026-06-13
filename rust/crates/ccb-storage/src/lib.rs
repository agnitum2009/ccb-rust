pub mod atomic;
pub mod classification;
pub mod json;
pub mod jsonl;
pub mod locks;
pub mod path_helpers;
pub mod paths;
pub mod project_identity;
pub mod text_artifacts;

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("corrupt data: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
