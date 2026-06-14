//! Mirrors Python lib/terminal_runtime/tmux_backend_panes.py
// TODO: translate from Python

use crate::backend;

/// Pane management for tmux backend
pub struct TmuxBackendPanes {
    backend: backend::TmuxBackend,
}

impl TmuxBackendPanes {
    pub fn new(backend: backend::TmuxBackend) -> Self {
        Self { backend }
    }

    pub fn list_panes(&self, session_name: &str) -> Result<Vec<crate::panes::PaneInfo>, Box<dyn std::error::Error>> {
        // TODO: implement pane listing
        Ok(Vec::new())
    }

    pub fn get_pane_info(&self, pane_id: &str) -> Result<crate::panes::PaneInfo, Box<dyn std::error::Error>> {
        // TODO: implement pane info retrieval
        Err("not implemented".into())
    }

    pub fn kill_pane(&self, pane_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement pane killing
        Ok(())
    }
}
