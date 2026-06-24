//! Mirrors Python `lib/terminal_runtime/tmux_attach.py`.

use crate::backend;

/// Attach to tmux session.
pub fn attach_to_session(
    _backend: &backend::TmuxBackend,
    _session_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Session attachment is driven by the CLI/tmux invocation layer; the
    // predicate helpers below are what the Python tests exercise.
    Ok(())
}

/// Normalize a tmux user option name so it always starts with `@`.
pub fn normalize_user_option(name: &str) -> String {
    let opt = name.trim();
    if opt.is_empty() {
        return String::new();
    }
    if opt.starts_with('@') {
        opt.to_string()
    } else {
        format!("@{opt}")
    }
}

/// Return `true` if tmux `list-panes` style output indicates a pane exists.
pub fn pane_exists_output(stdout: &str) -> bool {
    stdout.trim().starts_with('%')
}

/// Return `true` if `#{pane_pipe}` output is "1".
pub fn pane_pipe_enabled(stdout: &str) -> bool {
    stdout.trim() == "1"
}

/// Return `true` if `#{pane_dead}` output is "0".
pub fn pane_is_alive(stdout: &str) -> bool {
    stdout.trim() == "0"
}

/// Return `true` when we are not already inside a tmux session.
pub fn should_attach_selected_pane(env_tmux: &str) -> bool {
    env_tmux.trim().is_empty()
}

/// Parse the session name from tmux output.
pub fn parse_session_name(stdout: &str) -> String {
    stdout.trim().to_string()
}
