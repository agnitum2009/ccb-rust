//! Mirrors Python `test/test_v2_kill_service.py` remote-stop subset.

use ccbr_cli::context::{CliContext, CliContextBuilder};
use ccbr_cli::models::{ParsedCommand, ParsedKillCommand};
use ccbr_cli::services::daemon::DaemonHandle;
use ccbr_cli::services::daemon_runtime::models::KillSummary;
use ccbr_cli::services::kill_runtime::remote::{
    await_remote_shutdown, request_remote_stop, resolve_shutdown_summary,
};
use ccbr_cli::services::UnixDaemonClient;

fn make_context(tmp: &tempfile::TempDir) -> CliContext {
    let project_root = tmp.path();
    std::fs::create_dir_all(project_root.join(".ccbr")).unwrap();
    std::fs::write(project_root.join(".ccbr/ccbr.config"), "demo:codex\n").unwrap();
    CliContextBuilder::new(ParsedCommand::Kill(ParsedKillCommand {
        project: None,
        force: false,
        kind: "kill".into(),
    }))
    .cwd(project_root.to_path_buf())
    .build()
    .unwrap()
}

#[test]
fn test_request_remote_stop_returns_none_when_daemon_not_connectable() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);

    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let called_clone = called.clone();

    let result = request_remote_stop(
        &context,
        false,
        |_ctx, _allow_restart| Err(anyhow::anyhow!("no daemon")),
        |_ctx, _reason| {
            *called_clone.lock().unwrap() = true;
        },
        |_path, _timeout| unreachable!("client factory should not be called"),
        1.0,
    );

    assert!(result.unwrap().is_none());
    assert!(
        !*called.lock().unwrap(),
        "shutdown intent should not be recorded if daemon unreachable"
    );
}

#[test]
fn test_request_remote_stop_returns_none_on_client_failure_when_forced() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);

    let result = request_remote_stop(
        &context,
        true,
        |_ctx, _allow_restart| {
            Ok(DaemonHandle {
                client: UnixDaemonClient::new("/tmp/nonexistent.sock"),
            })
        },
        |_ctx, _reason| {},
        |_path, _timeout| Err(anyhow::anyhow!("factory failed")),
        1.0,
    );

    assert!(result.unwrap().is_none());
}

#[test]
fn test_resolve_shutdown_summary_prefers_remote_summary() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);

    let remote = KillSummary {
        project_id: context.project.project_id.clone(),
        state: "unmounted".into(),
        socket_path: "/tmp/sock".into(),
        forced: false,
        cleanup_summaries: Vec::new(),
        worktree_warnings: Vec::new(),
    };

    let result = resolve_shutdown_summary(
        &context,
        Some(&remote),
        false,
        |_ctx, _force| unreachable!("shutdown_daemon should not be called when remote exists"),
        |_ctx, _force| Ok(remote.clone()),
    )
    .unwrap();

    assert_eq!(result.project_id, context.project.project_id);
}

#[test]
fn test_resolve_shutdown_summary_falls_back_when_no_remote() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);

    let result = resolve_shutdown_summary(
        &context,
        None,
        false,
        |_ctx, _force| {
            Ok(KillSummary {
                project_id: context.project.project_id.clone(),
                state: "unmounted".into(),
                socket_path: "/tmp/sock".into(),
                forced: false,
                cleanup_summaries: Vec::new(),
                worktree_warnings: Vec::new(),
            })
        },
        |_ctx, _force| unreachable!("await_remote_shutdown should not be called when no remote"),
    )
    .unwrap();

    assert_eq!(result.project_id, context.project.project_id);
}

#[test]
fn test_await_remote_shutdown_polls_phase_and_terminates_lingering_pids() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);

    let phase = std::sync::Arc::new(std::sync::Mutex::new("mounted".to_string()));
    let phase_clone = phase.clone();

    let terminated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let terminated_clone = terminated.clone();

    let waited = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let waited_clone = waited.clone();

    let result = await_remote_shutdown(
        &context,
        false,
        0.5,
        &[123, 456],
        move |_ctx| phase_clone.lock().unwrap().clone(),
        |_pid| true,
        move |pid, _timeout, _is_alive| {
            terminated_clone.lock().unwrap().push(pid);
            true
        },
        move |pid, _timeout| {
            waited_clone.lock().unwrap().push(pid);
            false
        },
        |_timeout| true,
    )
    .unwrap();

    assert_eq!(result.project_id, context.project.project_id);
    assert_eq!(result.state, "mounted");
    let terminated = terminated.lock().unwrap();
    assert!(terminated.contains(&123));
    assert!(terminated.contains(&456));
    let waited = waited.lock().unwrap();
    assert!(waited.contains(&123));
    assert!(waited.contains(&456));
}

#[test]
fn test_await_remote_shutdown_stops_polling_when_unmounted() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);

    let result = await_remote_shutdown(
        &context,
        false,
        5.0,
        &[],
        |_ctx| "unmounted".to_string(),
        |_pid| false,
        |_pid, _timeout, _is_alive| true,
        |_pid, _timeout| true,
        |_timeout| true,
    )
    .unwrap();

    assert_eq!(result.state, "unmounted");
}
