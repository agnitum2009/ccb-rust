//! Mirrors Python `lib/cli/services/tmux_cleanup_history.py`.

use ccbr_storage::jsonl::JsonlStore;
use ccbr_storage::paths::PathLayout;
use serde::{Deserialize, Serialize};

use super::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;

const SCHEMA_VERSION: i32 = 1;

/// A single tmux cleanup event record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TmuxCleanupEvent {
    pub event_kind: String,
    pub project_id: String,
    pub occurred_at: String,
    pub summaries: Vec<ProjectTmuxCleanupSummary>,
}

impl TmuxCleanupEvent {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "tmux_cleanup_event",
            "event_kind": self.event_kind,
            "project_id": self.project_id,
            "occurred_at": self.occurred_at,
            "summaries": self.summaries.iter().map(|item| serde_json::json!({
                "socket_name": item.socket_name,
                "owned_panes": item.owned_panes,
                "active_panes": item.active_panes,
                "orphaned_panes": item.orphaned_panes,
                "killed_panes": item.killed_panes,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Append-only store for tmux cleanup events.
#[derive(Clone)]
pub struct TmuxCleanupHistoryStore {
    paths: PathLayout,
    store: JsonlStore,
}

impl TmuxCleanupHistoryStore {
    pub fn new(paths: PathLayout) -> Self {
        Self {
            paths,
            store: JsonlStore::new(),
        }
    }

    pub fn append(&self, event: &TmuxCleanupEvent) -> anyhow::Result<()> {
        self.store
            .append(
                &self.paths.ccbd_tmux_cleanup_history_path(),
                &event.to_record(),
            )
            .map_err(|e| anyhow::anyhow!("failed to append tmux cleanup event: {e}"))
    }

    pub fn load_latest(&self) -> anyhow::Result<Option<TmuxCleanupEvent>> {
        let rows: Vec<serde_json::Value> = self
            .store
            .read_all(&self.paths.ccbd_tmux_cleanup_history_path())
            .map_err(|e| anyhow::anyhow!("failed to read tmux cleanup history: {e}"))?;
        Ok(rows
            .into_iter()
            .last()
            .and_then(|record| serde_json::from_value::<TmuxCleanupEvent>(record).ok()))
    }
}
