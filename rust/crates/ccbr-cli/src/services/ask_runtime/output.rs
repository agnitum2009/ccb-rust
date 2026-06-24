//! Mirrors Python `lib/cli/services/ask_runtime/output.py`.

use std::path::Path;

use crate::output::{atomic_write_text, EXIT_ERROR, EXIT_NO_REPLY, EXIT_OK};

/// Map ask status to exit code.
///
/// Mirrors Python `exit_code_for_ask_status(status, reply)`.
pub fn exit_code_for_ask_status(status: Option<&str>, reply: &str) -> i32 {
    match status.unwrap_or("").trim().to_lowercase().as_str() {
        "completed" => EXIT_OK,
        "incomplete" => {
            if reply.is_empty() {
                EXIT_ERROR
            } else {
                EXIT_NO_REPLY
            }
        }
        _ => EXIT_ERROR,
    }
}

/// Write ask reply output to a file.
///
/// Mirrors Python `write_ask_output(path, reply)`.
pub fn write_ask_output(path: &Path, reply: &str) -> anyhow::Result<()> {
    let content = if reply.ends_with('\n') {
        reply.to_string()
    } else {
        format!("{}\n", reply)
    };
    atomic_write_text(path, &content)
}
