//! Mirrors Python `lib/cli/phase2_runtime/handlers_mailbox.py`.
//! 1:1 file alignment.

use serde_json::Value;
use std::io::Write;

use crate::phase2_runtime::handlers_ops::Phase2Services;
use crate::render_runtime::common::render_mapping;
use crate::render_runtime::common::render_observer_notice;
use crate::render_runtime::job_views::{
    render_cancel, render_resubmit, render_retry, render_wait, render_watch_batch,
};
use crate::render_runtime::mailbox_views::{
    render_ack, render_inbox, render_pend, render_queue, render_trace,
};

/// Handle the `ping` command.
///
/// Mirrors Python `handle_ping(context, command, out, services)`.
pub fn handle_ping<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.ping_target(context, command);
    services.write_lines(out, &render_mapping(&payload));
    0
}

/// Handle the `pend` command with observer mode support.
///
/// Mirrors Python `handle_pend(context, command, out, services)`.
pub fn handle_pend<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let observer_mode = command
        .get("observer_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("snapshot");

    match observer_mode {
        "watch" => {
            services.write_lines(out, &render_observer_notice("watch", false, "services"));
            let watch_command = serde_json::json!({
                "target": command.get("target")
            });
            for batch in services.watch_target(context, &watch_command) {
                services.write_lines(out, &render_watch_batch(&batch));
            }
            0
        }
        "inbox" => {
            let inbox_command = serde_json::json!({
                "agent_name": command.get("target"),
                "detail": command.get("detail").and_then(|v| v.as_bool()).unwrap_or(false)
            });
            let payload = services.inbox_target(context, &inbox_command);
            services.write_lines(out, &render_inbox(&payload));
            0
        }
        "queue" => {
            let queue_command = serde_json::json!({
                "target": command.get("target"),
                "detail": command.get("detail").and_then(|v| v.as_bool()).unwrap_or(false)
            });
            let payload = services.queue_target(context, &queue_command);
            services.write_lines(out, &render_queue(&payload));
            0
        }
        _ => {
            // Default snapshot mode
            let payload = services.pend_target(context, command);
            services.write_lines(out, &render_pend(&payload));
            0
        }
    }
}

/// Handle the `queue` command.
///
/// Mirrors Python `handle_queue(context, command, out, services)`.
pub fn handle_queue<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.queue_target(context, command);
    services.write_lines(out, &render_queue(&payload));
    0
}

/// Handle the `trace` command.
///
/// Mirrors Python `handle_trace(context, command, out, services)`.
pub fn handle_trace<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.trace_target(context, command);
    services.write_lines(out, &render_trace(&payload));
    0
}

/// Handle the `resubmit` command.
///
/// Mirrors Python `handle_resubmit(context, command, out, services)`.
pub fn handle_resubmit<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.resubmit_message(context, command);
    services.write_lines(out, &render_resubmit(&summary));
    0
}

/// Handle the `retry` command.
///
/// Mirrors Python `handle_retry(context, command, out, services)`.
pub fn handle_retry<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.retry_attempt(context, command);
    services.write_lines(out, &render_retry(&summary));
    0
}

/// Handle the `wait` command.
///
/// Mirrors Python `handle_wait(context, command, out, services)`.
pub fn handle_wait<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.wait_for_replies(context, command);
    services.write_lines(out, &render_wait(&summary));
    0
}

/// Handle the `inbox` command.
///
/// Mirrors Python `handle_inbox(context, command, out, services)`.
pub fn handle_inbox<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.inbox_target(context, command);
    services.write_lines(out, &render_inbox(&payload));
    0
}

/// Handle the `ack` command.
///
/// Mirrors Python `handle_ack(context, command, out, services)`.
pub fn handle_ack<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.ack_reply(context, command);
    services.write_lines(out, &render_ack(&payload));
    0
}

/// Handle the `watch` command.
///
/// Mirrors Python `handle_watch(context, command, out, services)`.
pub fn handle_watch<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    services.write_lines(out, &render_observer_notice("watch", false, "services"));
    for batch in services.watch_target(context, command) {
        services.write_lines(out, &render_watch_batch(&batch));
    }
    0
}

/// Handle the `cancel` command.
///
/// Mirrors Python `handle_cancel(context, command, out, services)`.
pub fn handle_cancel<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.cancel_job(context, command);
    services.write_lines(out, &render_cancel(&payload));
    0
}
