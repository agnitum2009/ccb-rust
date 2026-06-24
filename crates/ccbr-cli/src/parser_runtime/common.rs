//! Mirrors Python `lib/cli/parser_runtime/common.py`.
//!
//! Shared parser utilities: argument parsing helpers.
//! 1:1 alignment with Python functions.

/// Parse arguments using a clap-style parser, converting failures to a typed error.
///
/// Mirrors Python `parse_args(parser, tokens, error_message, error_type)`.
/// In Rust we return `Result` rather than raising; the caller maps to their error type.
pub fn parse_args<F, T, E>(parse_fn: F, tokens: &[&str], error_message: &str) -> Result<T, E>
where
    F: FnOnce(&[&str]) -> Result<T, String>,
    E: From<String>,
{
    parse_fn(tokens).map_err(|_| E::from(error_message.to_string()))
}

/// Ensure no extra tokens remain after parsing a command.
///
/// Mirrors Python `require_no_extra(tokens, command, error_type)`.
pub fn require_no_extra<E>(tokens: &[String], command: &str) -> Result<(), E>
where
    E: From<String>,
{
    if tokens.is_empty() {
        Ok(())
    } else {
        Err(E::from(format!(
            "{} does not accept extra arguments: {:?}",
            command, tokens
        )))
    }
}
