//! Mirrors Python `lib/cli/phase2_runtime/handlers_start.py`.
//! 1:1 file alignment.

use std::io::Write;
use serde_json::Value;

use crate::render_runtime::ops_views::render_config_validate;
use crate::render_runtime::ops_views::render_start;
use crate::phase2_runtime::handlers_ops::Phase2Services;
use crate::phase2_runtime::common::{env_truthy, stream_is_tty};

/// Handle the `config validate` command.
///
/// Mirrors Python `handle_config_validate(context, command, out, services)`.
pub fn handle_config_validate<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    _command: &Value,
) -> i32 {
    let summary = services.validate_config_context(context);
    services.write_lines(out, &render_config_validate(&summary));
    0
}

/// Handle the `start` command with optional interactive attach.
///
/// Mirrors Python `handle_start(context, command, out, services)`.
pub fn handle_start<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    // Determine if interactive attach should be used
    let interactive_attach = !env_truthy("CCB_NO_ATTACH") && stream_is_tty();

    // Get terminal size if interactive attach is enabled
    let terminal_size = if interactive_attach {
        get_terminal_size()
    } else {
        None
    };

    // Call start_agents with or without terminal_size
    let summary = if let Some((cols, rows)) = terminal_size {
        services.start_agents(context, command, Some((cols, rows)))
    } else {
        services.start_agents(context, command, None)
    };

    // If interactive attach, return early (Python calls attach_started_project_namespace)
    if interactive_attach {
        // NOTE: attach_started_project_namespace is handled by the caller in Python;
        // in Rust, this would be wired when the daemon runtime is available.
        return 0;
    }

    // Otherwise, render and write the start summary
    services.write_lines(out, &render_start(&summary));
    0
}

/// Get terminal size (columns, rows) if available.
///
/// Mirrors Python `_terminal_size_for_streams(*streams)`.
/// Returns None if terminal size cannot be determined.
fn get_terminal_size() -> Option<(u16, u16)> {
    // Try to get terminal size from stdout
    std::io::stdout()
        .terminal_size()
        .ok()
        .map(|size| (size.columns, size.rows))
}
