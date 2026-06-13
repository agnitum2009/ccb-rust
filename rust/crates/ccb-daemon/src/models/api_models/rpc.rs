use super::common::API_VERSION;
use serde::{Deserialize, Serialize};

/// RPC request that accepts either the Python `op`/`request` shape or the
/// `ccb-cli` `method`/`params` shape. This keeps the daemon compatible with
/// both the legacy Python protocol and the new Rust CLI client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcRequest {
    #[serde(default, alias = "method")]
    pub op: String,
    #[serde(default, alias = "params")]
    pub request: serde_json::Value,
    #[serde(default = "default_api_version")]
    pub api_version: u32,
}

fn default_api_version() -> u32 {
    API_VERSION
}

impl RpcRequest {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let mut value: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("invalid rpc request: {}", e))?;

        // Normalize ccb-cli shape (`method`/`params`) to daemon shape (`op`/`request`).
        if let Some(obj) = value.as_object_mut() {
            if let Some(method) = obj.remove("method") {
                if let Some(s) = method.as_str() {
                    obj.entry("op".to_string())
                        .or_insert_with(|| serde_json::json!(s));
                }
            }
            if let Some(params) = obj.remove("params") {
                obj.entry("request".to_string()).or_insert(params);
            }
        }

        serde_json::from_value(value).map_err(|e| format!("invalid rpc request: {}", e))
    }

    /// True if the original request used the `method`/`params` shape.
    pub fn uses_cli_shape(raw: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(raw)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .map(|obj| obj.contains_key("method") || obj.contains_key("params"))
            .unwrap_or(false)
    }
}

/// RPC response. Serializes in the Python-compatible flattened shape by
/// default. When responding to `ccb-cli` we also include a `result` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub ok: bool,
    #[serde(flatten)]
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default = "default_api_version")]
    pub api_version: u32,
}

impl RpcResponse {
    pub fn success(payload: serde_json::Value) -> Self {
        Self {
            ok: true,
            payload: payload.clone(),
            error: None,
            result: Some(payload),
            api_version: API_VERSION,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            payload: serde_json::json!({}),
            error: Some(error.into()),
            result: None,
            api_version: API_VERSION,
        }
    }

    /// Build a response intended for the Python-style protocol only.
    pub fn python_success(payload: serde_json::Value) -> Self {
        Self {
            ok: true,
            payload,
            error: None,
            result: None,
            api_version: API_VERSION,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"serialization failed"}"#.into())
    }
}
