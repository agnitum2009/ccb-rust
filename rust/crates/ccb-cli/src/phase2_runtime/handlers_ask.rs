//! Mirrors Python `lib/cli/phase2_runtime/handlers_ask.py`.
//! 1:1 file alignment.

use std::io::Write;
use serde_json::Value;

use crate::render_runtime::job_views::render_ask;
use crate::phase2_runtime::handlers_ops::Phase2Services;

/// Handle the `ask` command.
///
/// Mirrors Python `handle_ask(context, command, out, services)`.
pub fn handle_ask<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.submit_ask(context, command);
    services.write_lines(out, &render_ask(&summary));
    0
}
