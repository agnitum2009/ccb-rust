//! Mirrors Python lib/terminal_runtime/tmux_panes_runtime/queries.py
// TODO: translate from Python

use crate::backend;

/// Pane runtime queries
pub struct TmuxPaneQueries {
    backend: backend::TmuxBackend,
}

impl TmuxPaneQueries {
    pub fn new(backend: backend::TmuxBackend) -> Self {
        Self { backend }
    }

    pub fn list_panes(&self, session: &str) -> Result<Vec<crate::panes::PaneInfo>, Box<dyn std::error::Error>> {
        // TODO: implement pane listing
        Ok(Vec::new())
    }

    pub fn get_pane_info(&self, pane_id: &str) -> Result<crate::panes::PaneInfo, Box<dyn std::error::Error>> {
        // TODO: implement pane info retrieval
        Err("not implemented".into())
    }

    pub fn get_current_pane_id(&self) -> Result<String, Box<dyn std::error::Error>> {
        // TODO: implement current pane ID retrieval
        Ok(String::new())
    }
}
