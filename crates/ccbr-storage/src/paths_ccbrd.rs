use camino::Utf8PathBuf;

use crate::path_helpers::{choose_socket_placement, RootKind, SocketPlacement};
use crate::paths::PathLayout;

impl PathLayout {
    // --- Project anchor paths ---

    pub fn project_anchor_dir(&self) -> Utf8PathBuf {
        self.ccbr_dir()
    }

    pub fn ccbr_dir(&self) -> Utf8PathBuf {
        self.project_root.join(".ccbr")
    }

    pub fn config_path(&self) -> Utf8PathBuf {
        self.ccbr_dir().join("ccbr.config")
    }

    // --- CCBRD paths ---

    pub fn ccbrd_dir(&self) -> Utf8PathBuf {
        self.runtime_state_root.join("ccbrd")
    }

    pub fn ccbrd_submissions_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("submissions.jsonl")
    }

    pub fn ccbrd_mailboxes_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("mailboxes")
    }

    pub fn ccbrd_messages_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("messages")
    }

    pub fn ccbrd_messages_path(&self) -> Utf8PathBuf {
        self.ccbrd_messages_dir().join("messages.jsonl")
    }

    pub fn ccbrd_attempts_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("attempts")
    }

    pub fn ccbrd_attempts_path(&self) -> Utf8PathBuf {
        self.ccbrd_attempts_dir().join("attempts.jsonl")
    }

    pub fn ccbrd_replies_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("replies")
    }

    pub fn ccbrd_replies_path(&self) -> Utf8PathBuf {
        self.ccbrd_replies_dir().join("replies.jsonl")
    }

    pub fn ccbrd_callback_edges_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("callbacks/edges.jsonl")
    }

    pub fn ccbrd_leases_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("leases")
    }

    pub fn ccbrd_dead_letters_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("dead-letters")
    }

    pub fn ccbrd_dead_letters_path(&self) -> Utf8PathBuf {
        self.ccbrd_dead_letters_dir().join("dead_letters.jsonl")
    }

    pub fn ccbrd_provider_health_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("provider-health")
    }

    // --- Message bureau paths (legacy aliases used by other crates) ---

    pub fn message_bureau_dir(&self) -> Utf8PathBuf {
        self.ccbrd_messages_dir()
    }

    pub fn message_store_path(&self) -> Utf8PathBuf {
        self.ccbrd_messages_path()
    }

    pub fn attempt_store_path(&self) -> Utf8PathBuf {
        self.ccbrd_attempts_path()
    }

    pub fn reply_store_path(&self) -> Utf8PathBuf {
        self.ccbrd_replies_path()
    }

    // --- CCBRD mount / lifecycle paths ---

    fn project_socket_placement(&self, stem: &str) -> SocketPlacement {
        let preferred_root_kind =
            if matches!(self.runtime_state_placement.root_kind, RootKind::Relocated) {
                RootKind::Runtime
            } else {
                RootKind::Project
            };
        choose_socket_placement(
            &self.ccbrd_dir().join(format!("{}.sock", stem)),
            &self.project_socket_key(),
            preferred_root_kind,
        )
    }

    pub fn ccbrd_socket_placement(&self) -> SocketPlacement {
        self.project_socket_placement("ccbrd")
    }

    pub fn ccbrd_socket_path(&self) -> Utf8PathBuf {
        self.ccbrd_socket_placement().effective_path
    }

    pub fn ccbrd_pid_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("ccbrd.pid")
    }

    pub fn ccbrd_lifecycle_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("lifecycle.json")
    }

    pub fn ccbrd_lease_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("lease.json")
    }

    pub fn ccbrd_state_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("state.json")
    }

    pub fn ccbrd_project_view_state_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("project-view-state.json")
    }

    pub fn ccbrd_start_policy_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("start-policy.json")
    }

    pub fn ccbrd_restore_report_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("restore-report.json")
    }

    pub fn ccbrd_startup_report_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("startup-report.json")
    }

    pub fn ccbrd_shutdown_report_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("shutdown-report.json")
    }

    pub fn ccbrd_tmux_socket_placement(&self) -> SocketPlacement {
        self.project_socket_placement("tmux")
    }

    pub fn ccbrd_tmux_socket_path(&self) -> Utf8PathBuf {
        self.ccbrd_tmux_socket_placement().effective_path
    }

    pub fn ccbrd_tmux_session_name(&self) -> String {
        let safe = crate::paths::tmux_safe_name(&self.project_slug(), "project");
        format!("ccbr-{}", safe)
    }

    pub fn ccbrd_tmux_control_window_name(&self) -> &'static str {
        "__ccbr_ctl"
    }

    pub fn ccbrd_tmux_workspace_window_name(&self) -> &'static str {
        "ccbr"
    }

    // --- CCBRD ops paths ---

    pub fn ccbrd_supervision_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("supervision.jsonl")
    }

    pub fn ccbrd_lifecycle_log_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("lifecycle.jsonl")
    }

    pub fn ccbrd_keeper_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("keeper.json")
    }

    pub fn ccbrd_shutdown_intent_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("shutdown-intent.json")
    }

    pub fn ccbrd_tmux_cleanup_history_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("tmux-cleanup-history.jsonl")
    }

    pub fn ccbrd_maintenance_heartbeat_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("maintenance-heartbeat")
    }

    pub fn ccbrd_maintenance_heartbeat_schedule_path(&self) -> Utf8PathBuf {
        self.ccbrd_maintenance_heartbeat_dir().join("schedule.json")
    }

    pub fn ccbrd_maintenance_heartbeat_status_path(&self) -> Utf8PathBuf {
        self.ccbrd_maintenance_heartbeat_dir().join("status.json")
    }

    pub fn ccbrd_maintenance_heartbeat_runner_path(&self) -> Utf8PathBuf {
        self.ccbrd_maintenance_heartbeat_dir().join("runner.json")
    }

    pub fn ccbrd_maintenance_heartbeat_lock_path(&self) -> Utf8PathBuf {
        self.ccbrd_maintenance_heartbeat_dir().join("lock.json")
    }

    pub fn ccbrd_maintenance_heartbeat_activations_path(&self) -> Utf8PathBuf {
        self.ccbrd_maintenance_heartbeat_dir()
            .join("activations.jsonl")
    }

    pub fn ccbrd_fault_injection_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("fault-injection.json")
    }

    pub fn ccbrd_reload_drain_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("reload-drain.json")
    }

    pub fn ccbrd_reload_handoff_path(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("reload-handoff.json")
    }

    // --- CCBRD artifact paths ---

    pub fn ccbrd_artifacts_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("artifacts")
    }

    pub fn ccbrd_text_artifacts_dir(&self) -> Utf8PathBuf {
        self.ccbrd_artifacts_dir().join("text")
    }

    pub fn ccbrd_support_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("support")
    }

    pub fn ccbrd_executions_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("executions")
    }

    pub fn ccbrd_snapshots_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("snapshots")
    }

    pub fn ccbrd_cursors_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("cursors")
    }

    pub fn ccbrd_heartbeats_dir(&self) -> Utf8PathBuf {
        self.ccbrd_dir().join("heartbeats")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ccbr_dir_structure() {
        let layout = PathLayout::new("/project");
        assert_eq!(layout.ccbr_dir(), Utf8PathBuf::from("/project/.ccbr"));
        assert_eq!(
            layout.ccbrd_dir(),
            Utf8PathBuf::from("/project/.ccbr/ccbrd")
        );
    }
}
