//! Mirrors Python `lib/cli/ask_syntax.py`.
//!
//! Ask command route parsing: extracts target, optional sender, and message.
//! 1:1 alignment with Python dataclass + function.

use serde::{Deserialize, Serialize};

/// Parsed route components for an `ask` command.
///
/// Mirrors Python `ParsedAskRoute` frozen dataclass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedAskRoute {
    pub target: String,
    pub sender: Option<String>,
    pub message: String,
}

/// Parse ask route tokens into structured components.
///
/// Grammar: `<target> [from <sender>] [--] <message...>`
///
/// Mirrors Python `parse_ask_route`.
pub fn parse_ask_route(tokens: &[String], command_name: &str) -> Result<ParsedAskRoute, String> {
    let mut remaining: Vec<String> = tokens.to_vec();

    if remaining.len() < 2 {
        return Err(format!(
            "{} requires <target> [from <sender>] <message>",
            command_name
        ));
    }

    let target = remaining.remove(0).trim().to_string();
    if target.is_empty() {
        return Err(format!("{} target cannot be empty", command_name));
    }

    let mut sender: Option<String> = None;
    if remaining.first().map(|s| s.as_str()) == Some("from") {
        if remaining.len() < 3 {
            return Err(format!(
                "{} requires <target> [from <sender>] <message>",
                command_name
            ));
        }
        remaining.remove(0); // consume "from"
        let s = remaining.remove(0).trim().to_string();
        if s.is_empty() {
            return Err(format!("{} sender cannot be empty", command_name));
        }
        sender = Some(s);
    }

    // Skip optional "--" separator
    if remaining.first().map(|s| s.as_str()) == Some("--") {
        remaining.remove(0);
    }

    let message = remaining
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    if message.is_empty() {
        return Err(format!("{} message cannot be empty", command_name));
    }

    Ok(ParsedAskRoute {
        target,
        sender,
        message,
    })
}
