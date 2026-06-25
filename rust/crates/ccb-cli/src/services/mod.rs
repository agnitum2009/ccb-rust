use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// Generic RPC client interface for the CCB daemon.
pub trait DaemonClient: Send + Sync {
    /// Invoke a daemon RPC and return the `result` payload on success.
    fn call(&self, method: &str, params: Value) -> Result<Value, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Unix-socket based daemon client.
#[derive(Debug, Clone)]
pub struct UnixDaemonClient {
    socket_path: String,
    timeout_s: Option<f64>,
}

impl UnixDaemonClient {
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            timeout_s: None,
        }
    }

    pub fn with_timeout(mut self, timeout_s: f64) -> Self {
        self.timeout_s = Some(timeout_s);
        self
    }

    pub fn timeout_s(&self) -> Option<f64> {
        self.timeout_s
    }
}

impl DaemonClient for UnixDaemonClient {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| format!("cannot connect to daemon at {}: {}", self.socket_path, e))?;

        let request = RpcRequest {
            method: method.into(),
            params,
        };
        let json =
            serde_json::to_string(&request).map_err(|e| format!("serialize error: {}", e))?;

        stream
            .write_all(json.as_bytes())
            .map_err(|e| format!("write error: {}", e))?;
        stream
            .write_all(b"\n")
            .map_err(|e| format!("write error: {}", e))?;

        let mut reader = BufReader::new(&stream);
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| format!("read error: {}", e))?;

        let resp: RpcResponse =
            serde_json::from_str(&response).map_err(|e| format!("parse error: {}", e))?;

        if resp.ok {
            resp.result
                .ok_or_else(|| "daemon returned empty result".to_string())
        } else {
            Err(resp.error.unwrap_or_else(|| "daemon error".to_string()))
        }
    }
}

/// Build the daemon socket path for a project root.
pub fn socket_path_for_project(project_root: &Path) -> String {
    let layout = ccb_storage::paths::PathLayout::new(
        camino::Utf8Path::from_path(project_root).unwrap_or(camino::Utf8Path::new("/")),
    );
    layout.ccbd_socket_path().to_string()
}

/// Resolve the project root from the current directory and optional `--project` flag.
pub fn resolve_project_root(cwd: &Path, project_flag: Option<&str>) -> Result<PathBuf, String> {
    if let Some(raw) = project_flag {
        let expanded = expand_user_path(raw);
        let path = PathBuf::from(&expanded);
        let path = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        return Ok(path);
    }

    let mut current = cwd.to_path_buf();
    loop {
        if current.join(".ccb").is_dir() {
            return Ok(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => {
                return Err(
                    "not inside a CCB project (no .ccb directory found); use --project"
                        .to_string(),
                )
            }
        }
    }
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_request_serde() {
        let req = RpcRequest {
            method: "ping".into(),
            params: serde_json::json!({}),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("ping"));
    }

    #[test]
    fn test_rpc_response_serde() {
        let resp = RpcResponse {
            ok: true,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: RpcResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.ok);
    }
}

pub mod ack;
pub mod ask;
pub mod ask_runtime;
pub mod cancel;
pub mod cleanup;
pub mod clear;
pub mod config_validate;
pub mod daemon;
pub mod daemon_runtime;
pub mod diagnostics;
pub mod diagnostics_runtime;
pub mod doctor;
pub mod doctor_runtime;
pub mod doctor_storage;
pub mod fault;
pub mod inbox;
pub mod kill;
pub mod kill_runtime;
pub mod logs;
pub mod maintenance;
pub mod pend;
pub mod ping;
pub mod provider_binding;
pub mod provider_hooks;
pub mod ps;
pub mod queue;
pub mod reload;
pub mod reload_handoff;
pub mod reset_project;
pub mod restart;
pub mod resubmit;
pub mod retry;
pub mod role_lock_refresh;
pub mod runtime_launch;
pub mod start;
pub mod start_foreground;
pub mod start_runtime;
pub mod tmux_cleanup_history;
pub mod tmux_project_cleanup;
pub mod tmux_project_cleanup_runtime;
pub mod tmux_start_layout;
pub mod tmux_ui;
pub mod trace;
pub mod wait;
pub mod wait_runtime;
pub mod watch_fallback;
pub mod watch_runtime;
