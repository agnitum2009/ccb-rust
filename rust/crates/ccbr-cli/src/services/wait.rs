//! Mirrors Python `lib/cli/services/wait.py`.
//!
//! Public wait service entry point that wires the runtime implementation to the
//! real Unix-socket daemon client.

use crate::context::CliContext;
use crate::models::ParsedWaitCommand;
use crate::services::daemon::build_trace_client;
use crate::services::wait_runtime::models::WaitSummary;
use crate::services::wait_runtime::service::wait_for_replies as wait_impl;

/// Wait for replies to a message/target using the project daemon.
///
/// Mirrors Python `cli.services.wait.wait_for_replies`.
pub fn wait_for_replies(context: &CliContext, command: &ParsedWaitCommand) -> WaitSummary {
    let client = build_trace_client(context);
    wait_impl(
        context,
        command,
        &client,
        std::thread::sleep,
        std::time::Instant::now,
    )
}
