//! Restore-state construction.
//!
//! Mirrors `lib/ccbd/start_runtime/restore.py`.

use crate::models::{AgentRestoreState, RestoreMode, RestoreStatus};

/// Build an initial `AgentRestoreState` from an effective restore mode name.
///
/// Mirrors Python `build_restore_state`.
pub fn build_restore_state(mode: &str) -> AgentRestoreState {
    let mode = mode.trim().to_lowercase();
    let status = match mode.as_str() {
        "fresh" => RestoreStatus::Fresh,
        "provider" => RestoreStatus::Provider,
        _ => RestoreStatus::Checkpoint,
    };
    let restore_mode = match mode.as_str() {
        "fresh" => RestoreMode::Fresh,
        "provider" => RestoreMode::Provider,
        _ => RestoreMode::Auto,
    };

    AgentRestoreState {
        restore_mode,
        last_checkpoint: None,
        conversation_summary: "bootstrap placeholder".into(),
        open_tasks: Vec::new(),
        files_touched: Vec::new(),
        base_commit: None,
        head_commit: None,
        last_restore_status: Some(status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_restore_state_maps_modes() {
        let fresh = build_restore_state("fresh");
        assert_eq!(fresh.restore_mode, RestoreMode::Fresh);
        assert_eq!(fresh.last_restore_status, Some(RestoreStatus::Fresh));

        let provider = build_restore_state("provider");
        assert_eq!(provider.restore_mode, RestoreMode::Provider);
        assert_eq!(provider.last_restore_status, Some(RestoreStatus::Provider));

        let auto = build_restore_state("auto");
        assert_eq!(auto.restore_mode, RestoreMode::Auto);
        assert_eq!(auto.last_restore_status, Some(RestoreStatus::Checkpoint));
    }
}
