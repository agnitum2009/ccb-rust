//! Mirrors Python `lib/cli/services/wait_runtime/service.py`.

use serde_json::Value;

use crate::context::CliContext;
use crate::models::ParsedWaitCommand;

use super::models::WaitSummary;
use super::policy::{resolve_poll_interval, resolve_quorum, resolve_timeout};
use super::replies::latest_replies;

/// Client capable of performing a daemon `trace` lookup.
///
/// Mirrors the `client.trace(...)` call used by the Python implementation.
pub trait TraceClient {
    fn trace(&self, target: &str) -> Result<Value, String>;
}

/// Poll the daemon until enough replies arrive for the wait target.
///
/// Mirrors Python `wait_for_replies` in `lib/cli/services/wait_runtime/service.py`.
/// The caller supplies a `TraceClient` so tests can inject a fake daemon.
pub fn wait_for_replies<S, C, M>(
    context: &CliContext,
    command: &ParsedWaitCommand,
    client: &C,
    sleep_fn: S,
    monotonic_fn: M,
) -> WaitSummary
where
    S: Fn(std::time::Duration),
    C: TraceClient,
    M: Fn() -> std::time::Instant,
{
    let timeout_s = resolve_timeout(command.timeout_s);
    let poll_interval_s = resolve_poll_interval();
    let started_at = monotonic_fn();
    let deadline = started_at + std::time::Duration::from_secs_f64(timeout_s);

    loop {
        match client.trace(&command.target) {
            Ok(payload) => {
                let (expected_count, replies, terminal_count, notice_count) =
                    latest_replies(&payload);
                if expected_count == 0 {
                    panic!("wait target has no attempt routes: {}", command.target);
                }
                let quorum = resolve_quorum(command, expected_count);
                if replies.len() >= quorum {
                    let waited_s = (monotonic_fn() - started_at).as_secs_f64();
                    let wait_status = if terminal_count >= quorum {
                        "satisfied".to_string()
                    } else {
                        "notice".to_string()
                    };
                    return WaitSummary {
                        wait_status,
                        project_id: context.project.project_id.clone(),
                        mode: command.mode.clone(),
                        target: command.target.clone(),
                        resolved_kind: payload
                            .get("resolved_kind")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        expected_count,
                        received_count: replies.len(),
                        terminal_count,
                        notice_count,
                        waited_s,
                        replies,
                    };
                }
                if monotonic_fn() >= deadline {
                    panic!(
                        "wait {} timed out for target {}",
                        command.mode, command.target
                    );
                }
                sleep_fn(std::time::Duration::from_secs_f64(poll_interval_s));
            }
            Err(_) => {
                if monotonic_fn() >= deadline {
                    panic!(
                        "wait {} timed out for target {}",
                        command.mode, command.target
                    );
                }
                sleep_fn(std::time::Duration::from_secs_f64(poll_interval_s));
            }
        }
    }
}
