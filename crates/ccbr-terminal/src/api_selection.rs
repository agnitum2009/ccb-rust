//! Mirrors Python lib/terminal_runtime/api_selection.py
// TODO: translate from Python

use crate::backend;
use crate::layouts;

/// Resolve backend for terminal type
pub fn resolve_backend(
    cached_backend: Option<backend::TmuxBackend>,
    terminal_type: Option<String>,
    detect_terminal_fn: impl Fn() -> Option<String>,
    tmux_backend_factory: impl Fn() -> backend::TmuxBackend,
) -> Option<backend::TmuxBackend> {
    // TODO: implement backend selection logic
    None
}

/// Resolve backend for session
pub fn resolve_backend_for_session(
    session_data: &crate::registry::UserSession,
    detect_terminal_fn: impl Fn() -> Option<String>,
    tmux_backend_factory: impl Fn() -> backend::TmuxBackend,
) -> backend::TmuxBackend {
    // TODO: implement session-based backend resolution
    tmux_backend_factory()
}

/// Resolve pane ID from session
pub fn resolve_pane_id_from_session(session_data: &crate::registry::UserSession) -> Option<String> {
    session_data
        .pane_id
        .clone()
        .or_else(|| session_data.tmux_session.clone())
}

/// Create layout
pub fn create_layout(
    providers: Vec<String>,
    cwd: &str,
    root_pane_id: Option<String>,
    tmux_session_name: Option<String>,
    percent: usize,
    set_markers: bool,
    marker_prefix: &str,
    tmux_backend_factory: impl Fn() -> backend::TmuxBackend,
    detached_session_name_fn: impl Fn() -> String,
) -> layouts::LayoutResult {
    // TODO: implement layout creation
    layouts::LayoutResult {
        panes: std::collections::HashMap::new(),
        root_pane_id: String::new(),
        needs_attach: false,
        created_panes: Vec::new(),
    }
}
