//! Mirrors Python `lib/ccbrd/api_models.py`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// RPC request sent from a `CcbdClient` to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub op: String,
    pub request: Value,
}

/// RPC response returned by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub ok: bool,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

impl RpcResponse {
    /// Reconstruct a response from a JSON value record.
    pub fn from_record(record: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(record)
    }
}

/// Abstraction for objects that can be serialized into an RPC payload.
pub trait MessageEnvelope: Send + Sync {
    fn to_record(&self) -> Value;
}
