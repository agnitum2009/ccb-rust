//! Mirrors Python lib/terminal_runtime/tmux_send.py
// TODO: translate from Python

use crate::backend;

/// Send text to tmux pane
pub fn send_text(
    backend: &backend::TmuxBackend,
    pane_id: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::backend::TerminalBackend;
    backend.send_text(pane_id, text).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
