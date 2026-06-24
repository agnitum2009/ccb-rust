//! Mirrors Python `test/test_ccbrd_tmux_state.py`.

use ccbr_daemon::services::health_assessment::tmux_runtime::backend::{
    OwnershipResult, TmuxBackend,
};
use ccbr_daemon::services::health_assessment::tmux_runtime::state::tmux_pane_state;

struct MissingBackend;
impl TmuxBackend for MissingBackend {
    fn pane_exists(&self, _pane_id: &str) -> bool {
        false
    }
}

struct ForeignBackend;
impl TmuxBackend for ForeignBackend {
    fn pane_exists(&self, _pane_id: &str) -> bool {
        true
    }
    fn inspect_ownership(&self, _pane_id: &str) -> OwnershipResult {
        OwnershipResult { is_owned: false }
    }
}

struct AliveBackend;
impl TmuxBackend for AliveBackend {
    fn pane_exists(&self, _pane_id: &str) -> bool {
        true
    }
    fn is_tmux_pane_alive(&self, _pane_id: &str) -> Option<bool> {
        Some(true)
    }
    fn inspect_ownership(&self, _pane_id: &str) -> OwnershipResult {
        OwnershipResult { is_owned: true }
    }
}

#[test]
fn test_tmux_pane_state_returns_missing_when_pane_is_absent() {
    assert_eq!(tmux_pane_state(&(), Some(&MissingBackend), "%1"), "missing");
}

#[test]
fn test_tmux_pane_state_returns_foreign_when_ownership_mismatches() {
    assert_eq!(tmux_pane_state(&(), Some(&ForeignBackend), "%1"), "foreign");
}

#[test]
fn test_tmux_pane_state_prefers_tmux_alive_method() {
    assert_eq!(tmux_pane_state(&(), Some(&AliveBackend), "%1"), "alive");
}
