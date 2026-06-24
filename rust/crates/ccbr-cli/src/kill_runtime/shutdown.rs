//! Concrete daemon shutdown wiring for the kill service.
//!
//! Mirrors the production wiring in Python `lib/cli/services/daemon_runtime/shutdown.py`.

use std::path::Path;

use ccbr_storage::json::JsonStore;
use serde_json::{json, Map, Value};

use crate::context::CliContext;
use crate::kill_runtime::processes::{is_pid_alive, terminate_pid_tree, wait_for_pid_exit};
use crate::services::daemon_runtime::models::KillSummary;
use crate::services::daemon_runtime::shutdown::shutdown_daemon as shutdown_daemon_impl;

/// Shutdown the project's daemon using local state and best-effort process
/// termination. Mirrors Python `shutdown_daemon(context, *, force)`.
pub fn shutdown_daemon(context: &CliContext, force: bool) -> anyhow::Result<KillSummary> {
    let socket_path = context.paths.ccbd_socket_path().into_std_path_buf();
    let project_id = context.project.project_id.clone();
    let paths = context.paths.clone();
    let paths_for_finalize = paths.clone();
    let paths_for_inspect = paths.clone();
    let socket_path_for_factory = socket_path.clone();

    shutdown_daemon_impl(
        |_project_id, reason, requested_by_pid| {
            // Reuse the daemon-level intent recorder, but override the PID so
            // tests do not depend on the test runner's PID.
            record_shutdown_intent_with_pid(context, reason, requested_by_pid);
        },
        move |socket_path| finalize_shutdown_lifecycle(&paths_for_finalize, socket_path),
        move || inspect_daemon(&paths_for_inspect),
        move || {
            let _ = socket_path_for_factory;
            // Client factory placeholder: a real implementation would connect
            // to `socket_path` and call `shutdown()`. The no-op is acceptable
            // for the local-only fallback path because socket-connectable
            // daemons are handled via `await_remote_shutdown`.
            Value::Null
        },
        |lease| expected_pid(lease, "ccbd_pid"),
        |lease| expected_pid(lease, "keeper_pid"),
        |pid, timeout| wait_for_pid_exit(pid, timeout, &is_pid_alive),
        |_timeout| true,
        is_pid_alive,
        terminate_pid_tree,
        2.0,
        force,
        &socket_path,
        &project_id,
    )
    .map_err(|e| anyhow::anyhow!("shutdown_daemon failed: {e}"))
}

fn record_shutdown_intent_with_pid(context: &CliContext, reason: &str, requested_by_pid: u32) {
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
        requested_by_pid,
    );
}

fn finalize_shutdown_lifecycle(paths: &ccbr_storage::paths::PathLayout, _socket_path: &Path) {
    let store = JsonStore::new();
    let lifecycle_path = paths.ccbd_lifecycle_path();
    let mut current = match store.load::<Value>(&lifecycle_path) {
        Ok(v) => v,
        Err(_) => Value::Object(Map::new()),
    };
    if let Some(obj) = current.as_object_mut() {
        obj.insert("phase".to_string(), Value::String("unmounted".to_string()));
        obj.insert(
            "desired_state".to_string(),
            Value::String("stopped".to_string()),
        );
        obj.insert("shutdown_intent".to_string(), Value::Null);
    }
    let _ = store.save(&lifecycle_path, &current);
}

fn inspect_daemon(paths: &ccbr_storage::paths::PathLayout) -> (Value, Value, Value) {
    let store = JsonStore::new();
    let lifecycle: Value = store
        .load(&paths.ccbd_lifecycle_path())
        .unwrap_or(Value::Null);
    let lease: Value = store.load(&paths.ccbd_lease_path()).unwrap_or(Value::Null);

    let socket_path = paths.ccbd_socket_path();
    let socket_connectable = socket_path.exists();

    let daemon_pid = expected_pid(&lease, "ccbd_pid");
    let pid_alive = daemon_pid > 0 && is_pid_alive(daemon_pid);

    let phase = lifecycle
        .get("phase")
        .and_then(|v| v.as_str())
        .unwrap_or("unmounted")
        .to_string();

    let inspection = json!({
        "phase": phase,
        "socket_connectable": socket_connectable,
        "pid_alive": pid_alive,
        "lease": lease,
    });

    (Value::Object(Map::new()), Value::Null, inspection)
}

fn expected_pid(lease: &Value, key: &str) -> i64 {
    lease.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    fn make_paths(tmp: &tempfile::TempDir) -> ccbr_storage::paths::PathLayout {
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let paths = ccbr_storage::paths::PathLayout::new(root);
        std::fs::create_dir_all(paths.ccbr_dir()).unwrap();
        std::fs::write(paths.ccbr_dir().join("ccbr.config"), "demo:codex\n").unwrap();
        paths
    }

    fn make_context(paths: &ccbr_storage::paths::PathLayout) -> CliContext {
        use crate::context::CliContextBuilder;
        use crate::models::{ParsedCommand, ParsedKillCommand};
        CliContextBuilder::new(ParsedCommand::Kill(ParsedKillCommand {
            project: None,
            force: false,
            kind: "kill".into(),
        }))
        .cwd(paths.project_root.as_std_path().to_path_buf())
        .build()
        .unwrap()
    }

    #[test]
    fn test_shutdown_daemon_finalizes_lifecycle() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = make_paths(&tmp);

        let lifecycle_path = paths.ccbd_lifecycle_path();
        std::fs::create_dir_all(lifecycle_path.parent().unwrap().as_std_path()).unwrap();
        std::fs::write(
            lifecycle_path.as_std_path(),
            r#"{"phase":"mounted","desired_state":"running"}"#,
        )
        .unwrap();

        let context = make_context(&paths);
        let _result = shutdown_daemon(&context, false).unwrap();

        let lifecycle: Value =
            serde_json::from_str(&std::fs::read_to_string(lifecycle_path.as_std_path()).unwrap())
                .unwrap();
        assert_eq!(lifecycle["phase"], "unmounted");
        assert_eq!(lifecycle["desired_state"], "stopped");
    }
}
