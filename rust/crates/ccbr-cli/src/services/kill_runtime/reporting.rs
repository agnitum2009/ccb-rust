//! Mirrors Python `lib/cli/services/kill_runtime/reporting.py`.

use ccbr_agents::models::AgentRuntime;
use ccbr_storage::paths::PathLayout;
use serde_json::json;

use crate::services::daemon_runtime::models::KillSummary;
use crate::services::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;

/// Build a `KillSummary` from a daemon `stop_all` payload.
///
/// Mirrors Python `summary_from_stop_all_payload`.
pub fn summary_from_stop_all_payload(payload: &serde_json::Value) -> KillSummary {
    let cleanup_summaries: Vec<ProjectTmuxCleanupSummary> = payload
        .get("cleanup_summaries")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if !item.is_object() {
                        return None;
                    }
                    Some(ProjectTmuxCleanupSummary {
                        socket_name: item
                            .get("socket_name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        owned_panes: string_array(item.get("owned_panes")),
                        active_panes: string_array(item.get("active_panes")),
                        orphaned_panes: string_array(item.get("orphaned_panes")),
                        killed_panes: string_array(item.get("killed_panes")),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    KillSummary {
        project_id: payload
            .get("project_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        state: payload
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unmounted")
            .to_string(),
        socket_path: payload
            .get("socket_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        forced: payload
            .get("forced")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        cleanup_summaries,
        worktree_warnings: Vec::new(),
    }
}

fn string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Merge multiple cleanup summary groups into one list.
///
/// Mirrors Python `merge_cleanup_summaries`.
pub fn merge_cleanup_summaries(
    remote: &[ProjectTmuxCleanupSummary],
    local: &[ProjectTmuxCleanupSummary],
) -> Vec<ProjectTmuxCleanupSummary> {
    remote.iter().chain(local.iter()).cloned().collect()
}

/// Record a kill report for diagnostics/history.
///
/// Mirrors Python `record_kill_report`. The Rust version writes a lightweight
/// JSON report because the full daemon inspection/snapshot types are not
/// exposed to the CLI crate.
pub fn record_kill_report(
    paths: &PathLayout,
    trigger: &str,
    forced: bool,
    cleanup_summaries: &[ProjectTmuxCleanupSummary],
    configured_agent_names: &[String],
    extra_agent_dir_names: &[String],
) -> anyhow::Result<()> {
    let report = json!({
        "record_type": "kill_report",
        "schema_version": 1,
        "trigger": trigger,
        "forced": forced,
        "stopped_agents": configured_agent_names,
        "extra_agents": extra_agent_dir_names,
        "cleanup_summary": cleanup_summaries.iter().map(|s| serde_json::json!({
            "socket_name": s.socket_name,
            "owned_panes": s.owned_panes,
            "active_panes": s.active_panes,
            "orphaned_panes": s.orphaned_panes,
            "killed_panes": s.killed_panes,
        })).collect::<Vec<_>>(),
    });
    let path = paths.ccbrd_dir().join("kill-report.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    ccbr_storage::json::JsonStore::new()
        .save(&path, &report)
        .map_err(|e| anyhow::anyhow!("failed to save kill report: {e}"))
}

/// Build a runtime snapshot for the kill report.
///
/// Mirrors Python `snapshot_for_runtime`. Returns `None` until the daemon
/// snapshot types are available to the CLI crate.
pub fn snapshot_for_runtime(_runtime: Option<&AgentRuntime>) -> Option<serde_json::Value> {
    None
}
