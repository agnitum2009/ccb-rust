//! Mirrors Python `lib/cli/parser_runtime/constants.py`.
//!
//! CLI parser constants: subcommand names, option sets, mode mappings.
//! 1:1 alignment with Python module.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// All recognized top-level subcommands.
pub static SUBCOMMANDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "ask",
        "cancel",
        "clear",
        "cleanup",
        "kill",
        "ps",
        "ping",
        "watch",
        "pend",
        "queue",
        "trace",
        "resubmit",
        "retry",
        "wait-any",
        "wait-all",
        "wait-quorum",
        "inbox",
        "ack",
        "logs",
        "maintenance",
        "doctor",
        "repair",
        "config",
        "fault",
        "reload",
        "restart",
    ]
    .into_iter()
    .collect()
});

/// Ask options that consume the next token as a value.
pub static ASK_OPTIONS_WITH_VALUES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| ["--task-id", "--reply-to", "--mode"].into_iter().collect());

/// Ask options that are boolean flags (no value).
pub static ASK_FLAG_OPTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "--artifact-io",
        "--artifact-reply",
        "--artifact-request",
        "--callback",
        "--compact",
        "--silence",
    ]
    .into_iter()
    .collect()
});

/// Mapping from wait subcommand names to their mode string.
pub static WAIT_COMMAND_TO_MODE: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        [
            ("wait-any", "any"),
            ("wait-all", "all"),
            ("wait-quorum", "quorum"),
        ]
        .into_iter()
        .collect()
    });

/// Recognized ask job actions.
pub static ASK_JOB_ACTIONS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| ["get", "cancel"].into_iter().collect());
