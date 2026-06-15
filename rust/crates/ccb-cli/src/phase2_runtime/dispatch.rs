//! Mirrors Python `lib/cli/phase2_runtime/dispatch.py`.
//! 1:1 file alignment.

use std::io::Write;
use serde_json::Value;

use crate::context::CliContext;
use crate::phase2_runtime::handlers_ops::Phase2Services;
use crate::phase2_runtime::handlers_ask::handle_ask;
use crate::phase2_runtime::handlers_mailbox::{
    handle_ack, handle_cancel, handle_inbox, handle_pend, handle_ping, handle_queue,
    handle_resubmit, handle_retry, handle_trace, handle_wait, handle_watch,
};
use crate::phase2_runtime::handlers_ops::{
    handle_cleanup, handle_clear, handle_doctor, handle_fault_arm, handle_fault_clear,
    handle_fault_list, handle_kill, handle_logs, handle_maintenance, handle_ps, handle_reload,
    handle_restart,
};
use crate::phase2_runtime::handlers_start::{handle_config_validate, handle_start};

/// Dispatch a phase2 command to its handler.
///
/// Mirrors Python `dispatch(context, command, out, services) -> int`.
///
/// # Arguments
/// * `context` - CLI execution context
/// * `command` - Parsed command with `kind` field
/// * `out` - Output writer
/// * `services` - Phase2 service provider
///
/// # Returns
/// Exit code (0 for success, non-zero for errors)
///
/// # Errors
/// Returns exit code 2 if command kind is unsupported.
pub fn dispatch<S: Phase2Services, W: Write>(
    context: &CliContext,
    command: &Value,
    out: &mut W,
    services: &S,
) -> i32 {
    // Extract command kind
    let kind = command
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Route to appropriate handler based on command kind
    match kind {
        "ack" => handle_ack(services, out, context, command),
        "ask" => handle_ask(services, out, context, command),
        "cancel" => handle_cancel(services, out, context, command),
        "clear" => handle_clear(services, out, context, command),
        "cleanup" => handle_cleanup(services, out, context, command),
        "config-validate" => handle_config_validate(services, out, context, command),
        "doctor" => handle_doctor(services, out, context, command),
        "fault-arm" => handle_fault_arm(services, out, context, command),
        "fault-clear" => handle_fault_clear(services, out, context, command),
        "fault-list" => handle_fault_list(services, out, context),
        "inbox" => handle_inbox(services, out, context, command),
        "kill" => handle_kill(services, out, context, command),
        "logs" => handle_logs(services, out, context, command),
        "maintenance" => handle_maintenance(services, out, context, command),
        "pend" => handle_pend(services, out, context, command),
        "ping" => handle_ping(services, out, context, command),
        "ps" => handle_ps(services, out, context, command),
        "queue" => handle_queue(services, out, context, command),
        "reload" => handle_reload(services, out, context, command),
        "restart" => handle_restart(services, out, context, command),
        "resubmit" => handle_resubmit(services, out, context, command),
        "retry" => handle_retry(services, out, context, command),
        "start" => handle_start(services, out, context, command),
        "trace" => handle_trace(services, out, context, command),
        "wait" => handle_wait(services, out, context, command),
        "watch" => handle_watch(services, out, context, command),
        _ => {
            // Unsupported command
            writeln!(
                out,
                "command_status: unsupported\nerror: unsupported v2 command: {}",
                kind
            )
            .ok();
            2
        }
    }
}
