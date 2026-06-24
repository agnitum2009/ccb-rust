//! Mirrors Python `lib/cli/services/daemon.py`.
//!
//! Local daemon introspection helpers used by CLI service commands.

use serde_json::{json, Value};
use std::thread;
use std::time::Duration;

use crate::context::CliContext;
use crate::services::daemon_runtime::compat;
use crate::services::{socket_path_for_project, DaemonClient, UnixDaemonClient};
use ccb_storage::json::JsonStore;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Default RPC timeout for control-plane probe calls.
///
/// Mirrors Python `cli.services.daemon.CONTROL_PLANE_RPC_TIMEOUT_S`.
pub const CONTROL_PLANE_RPC_TIMEOUT_S: f64 = 15.0;

/// Default shutdown wait when replacing an incompatible daemon.
const INCOMPATIBLE_SHUTDOWN_WAIT_S: f64 = 2.0;

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

/// Ensure the daemon is started for the current project.
///
/// Mirrors Python `cli.services.daemon.ensure_daemon_started`. Currently this
/// connects to an already-mounted daemon; full keeper-spawn polling is a
/// planned follow-up.
pub fn ensure_daemon_started(
    context: &CliContext,
) -> Result<
    crate::services::daemon_runtime::models::DaemonHandle,
    crate::services::daemon_runtime::models::CcbdServiceError,
> {
    if let Ok(_handle) = connect_mounted_daemon(context, false) {
        return Ok(crate::services::daemon_runtime::models::DaemonHandle {
            client: None,
            inspection: json!({"phase": "mounted", "socket_connectable": true}),
            started: false,
        });
    }
    Err(crate::services::daemon_runtime::models::CcbdServiceError(
        "ccbd is not running; run `ccb` in this project first".to_string(),
    ))
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

/// Build a control-plane client for the daemon socket.
///
/// Mirrors Python `cli.services.daemon._build_control_plane_client`.
/// Runtime control-plane calls use the default blocking timeout.
pub fn build_control_plane_client(socket_path: impl Into<String>) -> UnixDaemonClient {
    UnixDaemonClient::new(socket_path)
}

/// Build a short-timeout probe client for compatibility checks.
///
/// Mirrors Python `cli.services.daemon._build_probe_control_plane_client`.
/// Probe calls use `CONTROL_PLANE_RPC_TIMEOUT_S` to avoid hanging on a
/// stalled daemon.
pub fn build_probe_control_plane_client(socket_path: impl Into<String>) -> UnixDaemonClient {
    UnixDaemonClient::new(socket_path).with_timeout(CONTROL_PLANE_RPC_TIMEOUT_S)
}

/// Connect to a compatible daemon, optionally restarting on mismatch.
///
/// Mirrors Python `cli.services.daemon._connect_compatible_daemon`.
/// Returns `None` when the socket is not connectable or the daemon is
/// incompatible and `restart_on_mismatch` is false.
pub fn connect_compatible_daemon(
    context: &CliContext,
    inspection: &Value,
    restart_on_mismatch: bool,
) -> Option<DaemonHandle> {
    let socket_connectable = inspection
        .get("socket_connectable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !socket_connectable {
        return None;
    }

    let socket_path = socket_path_for_project(context.paths.project_root.as_std_path());
    let config_payload = config_identity_payload(context);

    if compat::inspection_matches_project_config(inspection, || config_payload.clone()) {
        return Some(DaemonHandle {
            client: build_control_plane_client(&socket_path),
        });
    }

    let probe = build_probe_control_plane_client(&socket_path);
    let matches = compat::daemon_matches_project_config(
        || config_payload.clone(),
        |target| {
            probe
                .call("ping", json!({ "target": target }))
                .unwrap_or(Value::Null)
        },
    );

    if matches {
        return Some(DaemonHandle {
            client: build_control_plane_client(&socket_path),
        });
    }

    if !restart_on_mismatch {
        return None;
    }

    // Request a graceful shutdown and give the old daemon time to exit.
    let runtime = build_control_plane_client(&socket_path);
    let _ = runtime.call("stop-all", json!({ "force": false }));
    thread::sleep(Duration::from_secs_f64(INCOMPATIBLE_SHUTDOWN_WAIT_S));
    None
}

/// Build the identity payload used for daemon compatibility checks.
fn config_identity_payload(context: &CliContext) -> Value {
    match ccb_agents::config::load_project_config(&context.paths) {
        Ok(result) => {
            let identity =
                ccb_agents::config_identity::project_config_identity_payload(&result.config);
            json!({
                "known_agents": identity.known_agents,
                "config_signature": identity.config_signature,
            })
        }
        Err(_) => json!({"known_agents": Vec::<String>::new(), "config_signature": ""}),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ParsedCommand;
    use crate::services::RpcResponse;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    fn make_context(tmp: &tempfile::TempDir) -> CliContext {
        let project_root = tmp.path().to_path_buf();
        let ccb_dir = project_root.join(".ccb");
        std::fs::create_dir_all(&ccb_dir).unwrap();
        std::fs::write(ccb_dir.join("ccb.config"), "agent1:codex\n").unwrap();
        let command = ParsedCommand::Start(crate::models_start::ParsedStartCommand::new(
            None,
            Vec::new(),
            false,
            false,
        ));
        crate::context::CliContextBuilder::new(command)
            .cwd(project_root.clone())
            .build()
            .unwrap()
    }

    #[test]
    fn control_plane_client_has_no_timeout() {
        let client = build_control_plane_client("/tmp/ccbd.sock");
        assert_eq!(client.timeout_s(), None);
    }

    #[test]
    fn probe_control_plane_client_uses_short_timeout() {
        let client = build_probe_control_plane_client("/tmp/ccbd.sock");
        assert_eq!(client.timeout_s(), Some(CONTROL_PLANE_RPC_TIMEOUT_S));
    }

    #[test]
    fn connect_compatible_daemon_returns_none_when_not_connectable() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let inspection = json!({"socket_connectable": false, "phase": "mounted"});
        assert!(connect_compatible_daemon(&context, &inspection, false).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn connect_compatible_daemon_probes_then_connects_with_runtime_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let socket_path = context.paths.ccbd_socket_path().to_string();
        if let Some(parent) = std::path::Path::new(&socket_path).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        // Compute the config signature the daemon must return to be compatible.
        let config_result = ccb_agents::config::load_project_config(&context.paths).unwrap();
        let identity =
            ccb_agents::config_identity::project_config_identity_payload(&config_result.config);
        let signature = identity.config_signature;

        let listener = UnixListener::bind(&socket_path).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).unwrap();
                let response = serde_json::to_string(&RpcResponse {
                    ok: true,
                    result: Some(json!({"config_signature": signature})),
                    error: None,
                })
                .unwrap();
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(b"\n");
            }
        });

        let inspection = json!({"socket_connectable": true, "phase": "mounted"});
        let handle = connect_compatible_daemon(&context, &inspection, false).unwrap();

        assert_eq!(handle.client.timeout_s(), None);
    }
}
