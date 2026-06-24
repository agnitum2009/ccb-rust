//! Mirrors Python lib/terminal_runtime/placeholders.py
// TODO: translate from Python

/// Pane placeholder command body
pub const PANE_PLACEHOLDER_BODY: &str = "while :; do sleep 3600; done";

/// Get pane placeholder command
pub fn pane_placeholder_cmd() -> String {
    PANE_PLACEHOLDER_BODY.to_string()
}

/// Get pane placeholder argv
pub fn pane_placeholder_argv() -> Vec<String> {
    vec![
        "sh".to_string(),
        "-lc".to_string(),
        PANE_PLACEHOLDER_BODY.to_string(),
    ]
}
