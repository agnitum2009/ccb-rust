//! Mirrors Python `lib/provider_backends/claude/registry_runtime/logging.py`.

/// Write a line to the registry log.
///
/// In the Rust implementation this is currently a no-op; registry diagnostics are
/// emitted through the `tracing` instrumentation instead.
pub fn write_registry_log(_line: &str) {
    // The Python implementation appends to the ccbrd runtime log file.
    // Rust providers do not depend on the daemon runtime, so this is intentionally
    // left as a placeholder.
}
