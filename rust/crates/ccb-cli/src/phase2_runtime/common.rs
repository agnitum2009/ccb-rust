//! Mirrors Python `lib/cli/phase2_runtime/common.py`.
//! 1:1 file alignment.

use std::io::IsTerminal;

/// Check if argv looks like a `config` command (possibly preceded by `--project` flags).
///
/// Mirrors Python `looks_like_config_validate(argv)`.
pub fn looks_like_config_validate(argv: &[String]) -> bool {
    let mut tokens = argv.iter();
    let mut index = 0usize;

    // Skip over `--project` arguments (each consumes the flag and its value)
    while tokens.clone().nth(index).map(|s| s.as_str()) == Some("--project") {
        index += 2;
    }

    // Check if the next remaining token is "config"
    tokens.clone().nth(index).map(|s| s.as_str()) == Some("config")
}

/// Check if a stream is a TTY.
///
/// Mirrors Python `stream_is_tty(stream)`.
/// The Python version accepts any stream object and checks for an `isatty` method.
/// In Rust, we use the `IsTerminal` trait for equivalent functionality.
/// For simplicity in the CLI context, this checks stdout (the common case).
pub fn stream_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Check if an environment variable is truthy.
///
/// Mirrors Python `env_truthy(name)`.
/// Returns true if the env var (trimmed and lowercased) is one of:
/// "1", "true", "yes", "on".
pub fn env_truthy(name: &str) -> bool {
    let value = std::env::var(name)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    matches!(value.as_str(), "1" | "true" | "yes" | "on")
}
