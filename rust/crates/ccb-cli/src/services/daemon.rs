//! Mirrors Python `lib/cli/services/daemon.py`.
//!
//! Local daemon introspection helpers used by CLI service commands.

use serde_json::{json, Value};

use crate::context::CliContext;
use crate::services::{socket_path_for_project, DaemonClient, UnixDaemonClient};
use ccb_storage::json::JsonStore;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Errors that can occur when talking to the local daemon.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CcbdServiceError {
    #[error("daemon service error: {0}")]
    Message(String),
}

/// Local daemon mount state.
///
/// Mirrors the namespace returned by Python `ping_local_state`.
#[derive(Debug, Clone)]
pub struct LocalState {
    pub mount_state: String,
}

/// Probe the local daemon state for the current project.
///
/// Currently returns a placeholder "mounted" state; in the future this will
/// query the daemon socket for the authoritative lifecycle store value.
pub fn ping_local_state(_context: &CliContext) -> LocalState {
    LocalState {
        mount_state: "mounted".into(),
    }
}

/// Build a socket client for the current project's daemon.
pub fn build_trace_client(context: &CliContext) -> UnixDaemonClient {
    let socket_path = socket_path_for_project(context.paths.project_root.as_std_path());
    UnixDaemonClient::new(socket_path)
}

/// Handle to a connected daemon, mirroring Python's `connect_mounted_daemon`.
#[derive(Debug, Clone)]
pub struct DaemonHandle {
    pub client: UnixDaemonClient,
}

/// Connect to the daemon for the current project.
///
/// Mirrors Python `cli.services.daemon.connect_mounted_daemon`. Returns an
/// error if the daemon socket is not reachable so callers can fall back to
/// local shutdown.
pub fn connect_mounted_daemon(
    context: &CliContext,
    _allow_restart_stale: bool,
) -> anyhow::Result<DaemonHandle> {
    let socket_path = socket_path_for_project(context.paths.project_root.as_std_path());
    #[cfg(unix)]
    {
        UnixStream::connect(&socket_path)
            .map_err(|e| anyhow::anyhow!("daemon socket not reachable at {socket_path}: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        if !std::path::Path::new(&socket_path).exists() {
            return Err(anyhow::anyhow!(
                "daemon socket not reachable at {socket_path}"
            ));
        }
    }
    Ok(DaemonHandle {
        client: UnixDaemonClient::new(socket_path),
    })
}

/// Inspect the local daemon phase from the lifecycle store.
///
/// Mirrors the phase portion of Python `inspect_daemon`. Returns `"unmounted"`
/// when no lifecycle record exists.
pub fn inspect_daemon_phase(context: &CliContext) -> String {
    let store = JsonStore::new();
    let lifecycle_path = context.paths.ccbd_lifecycle_path();
    match store.load::<Value>(&lifecycle_path) {
        Ok(value) => value
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("unmounted")
            .to_string(),
        Err(_) => "unmounted".to_string(),
    }
}

/// Record shutdown intent for the current project.
///
/// Mirrors Python `cli.services.daemon.record_shutdown_intent`. Persists the
/// intent to the lifecycle store and the shutdown-intent store so the daemon
/// runtime and later diagnostics can observe it.
pub fn record_shutdown_intent(context: &CliContext, reason: &str) {
    let store = JsonStore::new();
    let lifecycle_path = context.paths.ccbd_lifecycle_path();
    let shutdown_path = context.paths.ccbd_shutdown_intent_path();
    let project_id = context.project.project_id.clone();

    crate::services::daemon_runtime::keeper::record_shutdown_intent(
        |_| store.load(&lifecycle_path).unwrap_or(Value::Null),
        |value| {
            let _ = store.save(&lifecycle_path, value);
        },
        |value| {
            let _ = store.save(&shutdown_path, value);
        },
        &project_id,
        reason,
        std::process::id(),
    );
}

/// `TraceClient` implementation for the Unix socket daemon client.
impl crate::services::wait_runtime::service::TraceClient for UnixDaemonClient {
    fn trace(&self, target: &str) -> Result<Value, String> {
        self.call("trace", json!({ "target": target }))
    }
}
