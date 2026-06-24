//! Mirrors Python `lib/ccbd/socket_client_runtime/errors.py`.

use std::io;

use thiserror::Error;

/// Error raised by the `CcbdClient` when a daemon RPC fails.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct CcbdClientError {
    pub message: String,
}

impl CcbdClientError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<io::Error> for CcbdClientError {
    fn from(err: io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<serde_json::Error> for CcbdClientError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(err.to_string())
    }
}
