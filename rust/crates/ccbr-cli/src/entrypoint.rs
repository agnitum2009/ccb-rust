//! Mirrors Python `lib/cli/entrypoint.py`.
//!
//! Re-exports the CLI entrypoint function.
//! Python original: `from .entrypoint_runtime import run_cli_entrypoint`
//!
//! Note: `entrypoint_runtime` is a separate B-class stub that will be filled
//! in a later batch. For now this module declares the public signature so
//! downstream code can reference it.

// Will become `pub use crate::entrypoint_runtime::run_cli_entrypoint;`
// once entrypoint_runtime is implemented. For now expose a placeholder.

use std::io::Write;

/// Run the full CLI entrypoint, returning an exit code.
///
/// Mirrors Python `run_cli_entrypoint(argv, *, version, script_root, cwd, stdout, stderr) -> int`.
pub fn run_cli_entrypoint(
    _argv: &[String],
    _version: &str,
    _script_root: &camino::Utf8Path,
    _cwd: &camino::Utf8Path,
    _stdout: &mut dyn Write,
    _stderr: &mut dyn Write,
) -> i32 {
    // TODO: delegate to entrypoint_runtime once implemented
    1
}
