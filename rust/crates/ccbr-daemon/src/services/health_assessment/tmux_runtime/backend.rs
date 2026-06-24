//! Mirrors Python `lib/ccbd/services/health_assessment/tmux_runtime/backend.py`.

/// Result of inspecting whether a tmux pane belongs to the current project
/// namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipResult {
    pub is_owned: bool,
}

/// Abstraction over a tmux backend used by health assessment.
///
/// Mirrors the duck-typed backend object passed to Python
/// `tmux_runtime.state.tmux_pane_state`.
pub trait TmuxBackend {
    /// Returns `true` if the pane currently exists in the tmux server.
    fn pane_exists(&self, pane_id: &str) -> bool;

    /// Returns `Some(true)` if the pane is alive, when the backend can answer
    /// that question directly for tmux panes.
    fn is_tmux_pane_alive(&self, _pane_id: &str) -> Option<bool> {
        None
    }

    /// Generic alive check fallback.
    fn is_alive(&self, _pane_id: &str) -> Option<bool> {
        None
    }

    /// Ownership check used by `inspect_tmux_pane_ownership`.
    fn inspect_ownership(&self, _pane_id: &str) -> OwnershipResult {
        OwnershipResult { is_owned: true }
    }
}
