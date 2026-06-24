//! Mirrors Python lib/terminal_runtime/layouts_models.py
// TODO: translate from Python

/// Layout result
#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub panes: std::collections::HashMap<String, String>,
    pub root_pane_id: String,
    pub needs_attach: bool,
    pub created_panes: Vec<String>,
}

// Re-export from layouts module
pub use crate::layouts::LayoutResult as _LayoutResult;
pub use crate::layouts::TmuxLayoutBackend;
