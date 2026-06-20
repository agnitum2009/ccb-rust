//! Mirrors Python `lib/cli/services/daemon.py`.
//!
//! Local daemon introspection helpers used by CLI service commands.

use serde_json::{json, Value};

use crate::context::CliContext;
use crate::services::{socket_path_for_project, DaemonClient, UnixDaemonClient};

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
/// Mirrors Python `cli.services.daemon.connect_mounted_daemon`.
pub fn connect_mounted_daemon(context: &CliContext, _allow_restart_stale: bool) -> DaemonHandle {
    DaemonHandle {
        client: build_trace_client(context),
    }
}

/// `TraceClient` implementation for the Unix socket daemon client.
impl crate::services::wait_runtime::service::TraceClient for UnixDaemonClient {
    fn trace(&self, target: &str) -> Result<Value, String> {
        self.call("trace", json!({ "target": target }))
    }
}
