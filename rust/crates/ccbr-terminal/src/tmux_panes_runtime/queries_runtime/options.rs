//! Mirrors Python lib/terminal_runtime/tmux_panes_runtime/queries_runtime/options.py
// TODO: translate from Python

/// Pane query options
#[derive(Debug, Clone, Default)]
pub struct PaneQueryOptions {
    pub include_dead: bool,
    pub include_active_only: bool,
    pub session_filter: Option<String>,
}

impl PaneQueryOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_include_dead(mut self, include: bool) -> Self {
        self.include_dead = include;
        self
    }

    pub fn with_active_only(mut self, active_only: bool) -> Self {
        self.include_active_only = active_only;
        self
    }

    pub fn with_session_filter(mut self, session: String) -> Self {
        self.session_filter = Some(session);
        self
    }
}
