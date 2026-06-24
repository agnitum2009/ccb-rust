//! Mirrors Python lib/terminal_runtime/tmux_backend_control.py
// TODO: translate from Python

use crate::backend;

/// Control tmux backend
pub struct TmuxBackendControl {
    backend: backend::TmuxBackend,
}

impl TmuxBackendControl {
    pub fn new(backend: backend::TmuxBackend) -> Self {
        Self { backend }
    }

    pub fn send_keys(&self, pane_id: &str, keys: &str) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement send keys
        Ok(())
    }

    pub fn copy_mode(&self, pane_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement copy mode
        Ok(())
    }

    pub fn cancel_copy_mode(&self, pane_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement cancel copy mode
        Ok(())
    }
}
