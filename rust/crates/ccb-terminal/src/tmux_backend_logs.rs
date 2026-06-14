//! Mirrors Python lib/terminal_runtime/tmux_backend_logs.py
// TODO: translate from Python

use crate::backend;

/// Log capture from tmux backend
pub struct TmuxBackendLogs {
    backend: backend::TmuxBackend,
}

impl TmuxBackendLogs {
    pub fn new(backend: backend::TmuxBackend) -> Self {
        Self { backend }
    }

    pub fn capture_pane_output(&self, pane_id: &str) -> Result<String, Box<dyn std::error::Error>> {
        // TODO: implement pane output capture
        Ok(String::new())
    }

    pub fn capture_session_logs(&self, session_name: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // TODO: implement session log capture
        Ok(Vec::new())
    }
}
