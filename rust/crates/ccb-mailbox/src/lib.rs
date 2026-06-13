pub mod bureau;
pub mod control_queue;
pub mod control_trace;
pub mod facade_recording;
pub mod facade_state;
pub mod jobs;
pub mod kernel;
pub mod models;
pub mod reply_metadata;
pub mod reply_payloads;
pub mod stores;
pub mod targets;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MailboxError {
    #[error("storage error: {0}")]
    Storage(#[from] ccb_storage::StorageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, MailboxError>;
