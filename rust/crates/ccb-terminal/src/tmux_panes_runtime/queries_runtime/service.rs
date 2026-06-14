//! Mirrors Python lib/terminal_runtime/tmux_panes_runtime/queries_runtime/service.py
// TODO: translate from Python

use crate::backend;
use super::options::PaneQueryOptions;

/// Pane query service
pub struct TmuxPaneQueryService {
    backend: backend::TmuxBackend,
    options: PaneQueryOptions,
}

impl TmuxPaneQueryService {
    pub fn new(backend: backend::TmuxBackend, options: PaneQueryOptions) -> Self {
        Self { backend, options }
    }

    pub fn query_panes(&self) -> Result<Vec<crate::panes::PaneInfo>, Box<dyn std::error::Error>> {
        // TODO: implement pane querying with options
        Ok(Vec::new())
    }

    pub fn find_pane_by_id(&self, pane_id: &str) -> Result<Option<crate::panes::PaneInfo>, Box<dyn std::error::Error>> {
        // TODO: implement pane finding by ID
        Ok(None)
    }

    pub fn find_panes_by_title(&self, title: &str) -> Result<Vec<crate::panes::PaneInfo>, Box<dyn std::error::Error>> {
        // TODO: implement pane finding by title
        Ok(Vec::new())
    }
}
