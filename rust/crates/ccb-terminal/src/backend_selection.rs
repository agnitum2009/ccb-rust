//! Mirrors Python lib/terminal_runtime/backend_selection.py
// TODO: translate from Python

use crate::backend;
use crate::layouts;

/// Terminal backend selection
#[derive(Debug, Default)]
pub struct TerminalBackendSelection {
    cached: Option<backend::TmuxBackend>,
}

impl TerminalBackendSelection {
    pub fn new() -> Self {
        Self { cached: None }
    }

    pub fn get_backend(&mut self) -> Option<&backend::TmuxBackend> {
        if self.cached.is_none() {
            self.cached = Some(backend::TmuxBackend::new(None, None));
        }
        self.cached.as_ref()
    }

    pub fn get_backend_for_session(&self, session: &crate::registry::UserSession) -> backend::TmuxBackend {
        let socket_name = session.tmux_socket_name.clone();
        let socket_path = session.tmux_socket_path.clone();
        backend::TmuxBackend::new(socket_name, socket_path)
    }

    pub fn get_pane_id_from_session(&self, session: &crate::registry::UserSession) -> Option<String> {
        session.pane_id.clone().or_else(|| session.tmux_session.clone())
    }
}

/// Terminal layout service
pub struct TerminalLayoutService {
    tmux_backend_factory: Box<dyn Fn() -> backend::TmuxBackend>,
    detached_session_name_fn: Box<dyn Fn() -> String>,
    env: Option<std::collections::HashMap<String, String>>,
}

impl TerminalLayoutService {
    pub fn new(
        tmux_backend_factory: Box<dyn Fn() -> backend::TmuxBackend>,
        detached_session_name_fn: Box<dyn Fn() -> String>,
        env: Option<std::collections::HashMap<String, String>>,
    ) -> Self {
        Self {
            tmux_backend_factory,
            detached_session_name_fn,
            env,
        }
    }

    pub fn create_auto_layout(
        &self,
        providers: Vec<String>,
        cwd: &str,
        root_pane_id: Option<String>,
        tmux_session_name: Option<String>,
        percent: usize,
        set_markers: bool,
        marker_prefix: &str,
    ) -> layouts::LayoutResult {
        // TODO: implement auto layout creation
        layouts::LayoutResult {
            panes: std::collections::HashMap::new(),
            root_pane_id: String::new(),
            needs_attach: false,
            created_panes: Vec::new(),
        }
    }
}
