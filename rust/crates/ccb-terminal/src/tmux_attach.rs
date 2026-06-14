//! Mirrors Python lib/terminal_runtime/tmux_attach.py
// TODO: translate from Python

use crate::backend;

/// Attach to tmux session
pub fn attach_to_session(
    backend: &backend::TmuxBackend,
    session_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: implement session attachment
    Ok(())
}

/// Check if should attach selected pane
pub fn should_attach_selected_pane(env_tmux: &str) -> bool {
    !env_tmux.trim().is_empty()
}
