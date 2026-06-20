//! Mirrors Python `lib/ccbd/services/health_assessment/tmux_runtime/state.py`.

use super::backend::TmuxBackend;
use super::ownership::inspect_tmux_pane_ownership;

/// Normalize a pane id the same way Python does.
pub fn normalized_pane_id(pane_id: &str) -> String {
    pane_id.trim().to_string()
}

/// Determine the state of a tmux pane for health assessment.
///
/// Mirrors Python `tmux_pane_state(session, backend, pane_id)`.
pub fn tmux_pane_state<S, B>(session: &S, backend: Option<&B>, pane_id: &str) -> String
where
    B: TmuxBackend,
{
    let pane_text = normalized_pane_id(pane_id);
    if pane_text.is_empty() {
        return "missing".to_string();
    }
    let Some(backend) = backend else {
        return "missing".to_string();
    };
    if let Some(state) = pane_existence_state(backend, &pane_text) {
        return state;
    }
    let ownership = inspect_tmux_pane_ownership(session, backend, &pane_text);
    if !ownership.is_owned {
        return "foreign".to_string();
    }
    if let Some(state) = pane_alive_state(backend, &pane_text) {
        return state;
    }
    "missing".to_string()
}

fn pane_existence_state<B: TmuxBackend>(backend: &B, pane_id: &str) -> Option<String> {
    if backend.pane_exists(pane_id) {
        None
    } else {
        Some("missing".to_string())
    }
}

fn pane_alive_state<B: TmuxBackend>(backend: &B, pane_id: &str) -> Option<String> {
    if let Some(alive) = backend.is_tmux_pane_alive(pane_id) {
        return Some(if alive { "alive" } else { "dead" }.to_string());
    }
    if let Some(alive) = backend.is_alive(pane_id) {
        return Some(if alive { "alive" } else { "dead" }.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::backend::{OwnershipResult, TmuxBackend};
    use super::tmux_pane_state;

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
}
