//! Mirrors Python `lib/ccbd/socket_client.py`.
//!
//! Daemon socket client for CLI commands.
//! Provides RPC interface to the ccbd daemon over Unix socket.

use std::path::{Path, PathBuf};

use serde_json::Value;

use ccbr_daemon::api_models::RpcRequest;
use ccbr_daemon::socket_client_runtime::errors::CcbdClientError;
use ccbr_daemon::socket_client_runtime::transport;

/// RPC client for the local CCB daemon.
#[derive(Debug, Clone)]
pub struct CcbdClient {
    socket_path: PathBuf,
    timeout_s: f64,
}

impl CcbdClient {
    /// Create a new daemon client with default timeout.
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            timeout_s: resolve_timeout(None),
        }
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get the timeout in seconds.
    pub fn timeout_s(&self) -> f64 {
        self.timeout_s
    }

    /// Create a new client with a custom timeout.
    pub fn with_timeout(&self, timeout_s: f64) -> Self {
        Self {
            socket_path: self.socket_path.clone(),
            timeout_s: resolve_timeout(Some(timeout_s)),
        }
    }

    /// Send an RPC request and return the payload on success.
    pub fn request(&self, op: &str, payload: &Value) -> Result<Value, CcbdClientError> {
        let req = RpcRequest {
            op: op.to_string(),
            request: payload.clone(),
        };
        let mut sock = transport::connect_socket_unix(&self.socket_path, self.timeout_s)?;
        transport::send_request(&mut sock, &req)?;
        let raw = transport::recv_response_line(&mut sock)?;
        if raw.is_empty() {
            return Err(CcbdClientError::new("empty response from ccbd"));
        }
        let response = transport::decode_response(&raw)?;
        if !response.ok {
            return Err(CcbdClientError::new(
                response
                    .error
                    .unwrap_or_else(|| "ccbd request failed".into()),
            ));
        }
        Ok(response.payload.unwrap_or(Value::Null))
    }

    /// Send a ping request to the daemon.
    pub fn ping(&self, _target: &str) -> Result<Value, CcbdClientError> {
        self.request("ping", &Value::Null)
    }

    /// Send a start request to the daemon.
    pub fn start(
        &self,
        agent_names: &[String],
        restore: bool,
        auto_permission: bool,
        terminal_size: Option<(u32, u32)>,
    ) -> Result<Value, CcbdClientError> {
        let mut params = serde_json::Map::new();
        params.insert(
            "agent_names".into(),
            Value::Array(
                agent_names
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
        params.insert("restore".into(), Value::Bool(restore));
        params.insert("auto_permission".into(), Value::Bool(auto_permission));
        if let Some((cols, rows)) = terminal_size {
            params.insert("terminal_width".into(), Value::Number(cols.into()));
            params.insert("terminal_height".into(), Value::Number(rows.into()));
        }
        self.request("start", &Value::Object(params))
    }

    /// Send a stop-all request to the daemon.
    pub fn stop_all(&self, force: bool) -> Result<Value, CcbdClientError> {
        self.request("stop-all", &serde_json::json!({"force": force}))
    }
}

/// Resolve timeout from explicit value or environment variable.
fn resolve_timeout(explicit: Option<f64>) -> f64 {
    if let Some(t) = explicit {
        if t.is_finite() && t >= 0.1 {
            return t;
        }
        return 3.0;
    }
    if let Ok(raw) = std::env::var("CCBR_CCBD_CLIENT_TIMEOUT_S") {
        if let Ok(t) = raw.parse::<f64>() {
            if t.is_finite() && t >= 0.1 {
                return t;
            }
        }
    }
    3.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_timeout_default() {
        std::env::remove_var("CCBR_CCBD_CLIENT_TIMEOUT_S");
        assert_eq!(resolve_timeout(None), 3.0);
    }

    #[test]
    fn test_resolve_timeout_from_env() {
        std::env::set_var("CCBR_CCBD_CLIENT_TIMEOUT_S", "5.0");
        assert_eq!(resolve_timeout(None), 5.0);
        std::env::remove_var("CCBR_CCBD_CLIENT_TIMEOUT_S");
    }

    #[test]
    fn test_resolve_timeout_explicit() {
        assert_eq!(resolve_timeout(Some(2.5)), 2.5);
    }

    #[test]
    fn test_resolve_timeout_floor() {
        // Values below 0.1s should default to 3.0s
        assert_eq!(resolve_timeout(Some(0.05)), 3.0);
    }

    #[test]
    fn test_resolve_timeout_invalid() {
        // Non-finite values should default to 3.0s
        assert_eq!(resolve_timeout(Some(f64::INFINITY)), 3.0);
        assert_eq!(resolve_timeout(Some(f64::NAN)), 3.0);
    }

    #[test]
    fn test_decode_ok_response() {
        let raw = r#"{"ok":true,"payload":{"status":"alive"}}"#;
        let response = transport::decode_response(raw).unwrap();
        assert!(response.ok);
        assert_eq!(response.payload.unwrap(), json!({"status":"alive"}));
    }

    #[test]
    fn test_decode_error_response() {
        let raw = r#"{"ok":false,"error":"daemon not running"}"#;
        let response = transport::decode_response(raw).unwrap();
        assert!(!response.ok);
        assert_eq!(response.error.unwrap(), "daemon not running");
    }

    #[test]
    fn test_decode_empty_payload_response() {
        let raw = r#"{"ok":true}"#;
        let response = transport::decode_response(raw).unwrap();
        assert!(response.ok);
        assert_eq!(response.payload, None);
    }

    #[test]
    fn test_client_creation() {
        let client = CcbdClient::new("/tmp/test.sock");
        assert_eq!(client.socket_path(), PathBuf::from("/tmp/test.sock"));
        assert_eq!(client.timeout_s(), 3.0);
    }

    #[test]
    fn test_client_with_timeout() {
        let client = CcbdClient::new("/tmp/test.sock").with_timeout(5.0);
        assert_eq!(client.timeout_s(), 5.0);
    }
}
