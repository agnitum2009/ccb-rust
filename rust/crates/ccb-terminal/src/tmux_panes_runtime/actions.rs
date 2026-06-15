//! Mirrors Python lib/terminal_runtime/tmux_panes_runtime/actions.py
// TODO: translate from Python

use crate::backend;

/// Pane runtime actions
pub struct TmuxPaneActions {
    backend: backend::TmuxBackend,
}

impl TmuxPaneActions {
    pub fn new(backend: backend::TmuxBackend) -> Self {
        Self { backend }
    }

    pub fn split_pane(
        &self,
        parent_id: &str,
        direction: &str,
        percent: u32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // TODO: implement pane splitting
        Ok(String::new())
    }

    pub fn kill_pane(&self, pane_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement pane killing
        Ok(())
    }

    pub fn resize_pane(
        &self,
        pane_id: &str,
        direction: &str,
        size: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement pane resizing
        Ok(())
    }
}
