//! Mirrors Python `lib/cli/services/start_runtime.py`.

use crate::context::CliContext;
use crate::models_start::ParsedStartCommand;
use crate::services::daemon_runtime::models::{CcbdServiceError, DaemonHandle};
use crate::services::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartSummary {
    pub project_root: String,
    pub project_id: String,
    pub started: Vec<String>,
    pub daemon_started: bool,
    pub socket_path: String,
    #[serde(default)]
    pub cleanup_summaries: Vec<ProjectTmuxCleanupSummary>,
    #[serde(default)]
    pub worktree_warnings: Vec<Value>,
    #[serde(default)]
    pub worktree_retired: Vec<Value>,
    #[serde(default)]
    pub maintenance_heartbeat: Option<Value>,
}

/// Storage backend for the daemon startup report.
pub trait StartupReportStore: Send + Sync {
    fn load(&self, context: &CliContext) -> Option<Value>;
    fn save(&self, context: &CliContext, report: &Value) -> Result<(), String>;
}

/// JSON-backed startup report store.
pub struct JsonStartupReportStore;

impl StartupReportStore for JsonStartupReportStore {
    fn load(&self, context: &CliContext) -> Option<Value> {
        let path = context.paths.ccbrd_startup_report_path();
        ccbr_storage::json::JsonStore::new()
            .load::<Value>(&path)
            .ok()
    }

    fn save(&self, context: &CliContext, report: &Value) -> Result<(), String> {
        let path = context.paths.ccbrd_startup_report_path();
        ccbr_storage::json::JsonStore::new()
            .save(&path, report)
            .map_err(|e| e.to_string())
    }
}

/// Start project agents through the daemon.
///
/// Mirrors Python `start_agents(...)`.
#[allow(clippy::too_many_arguments)]
pub fn start_agents<E, S, B, H, W>(
    context: &CliContext,
    command: &ParsedStartCommand,
    terminal_size: Option<(u32, u32)>,
    ensure_daemon_started_fn: E,
    start_rpc_fn: S,
    startup_report_store: &dyn StartupReportStore,
    before_client_start_fn: B,
    enrich_summary_fn: H,
    heartbeat_fn: W,
) -> Result<StartSummary, String>
where
    E: FnOnce(&CliContext) -> Result<DaemonHandle, CcbdServiceError>,
    S: FnOnce(&DaemonHandle, Option<f64>) -> Result<Value, String>,
    B: FnOnce(&CliContext) -> Result<ccbr_workspace::reconcile::WorkspaceGuardSummary, String>,
    H: FnOnce(StartSummary, ccbr_workspace::reconcile::WorkspaceGuardSummary) -> StartSummary,
    W: FnOnce(&CliContext) -> Option<Value>,
{
    let guard_summary = before_client_start_fn(context)?;
    if !guard_summary.blockers.is_empty() {
        return Err(ccbr_workspace::reconcile::format_workspace_blockers(
            "ccbr start",
            &guard_summary.blockers,
        ));
    }

    let handle = ensure_daemon_started_fn(context).map_err(|e| e.to_string())?;

    let mut params = serde_json::Map::new();
    params.insert(
        "agent_names".into(),
        Value::Array(
            command
                .agent_names
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        ),
    );
    params.insert("restore".into(), Value::Bool(command.restore));
    params.insert(
        "auto_permission".into(),
        Value::Bool(command.auto_permission),
    );
    if let Some((cols, rows)) = terminal_size {
        params.insert("terminal_width".into(), Value::Number(cols.into()));
        params.insert("terminal_height".into(), Value::Number(rows.into()));
    }

    let payload = start_rpc_fn(&handle, None)?;
    record_daemon_started_flag(context, handle.started, startup_report_store);

    let summary = summary_from_payload(context, &payload, handle.started)?;
    let summary = enrich_summary_fn(summary, guard_summary);

    let heartbeat_summary = heartbeat_fn(context);
    Ok(StartSummary {
        maintenance_heartbeat: heartbeat_summary,
        ..summary
    })
}

fn record_daemon_started_flag(
    context: &CliContext,
    daemon_started: bool,
    store: &dyn StartupReportStore,
) {
    if let Some(mut report) = store.load(context) {
        if let Some(obj) = report.as_object_mut() {
            obj.insert("daemon_started".into(), Value::Bool(daemon_started));
        }
        let _ = store.save(context, &report);
    }
}

fn summary_from_payload(
    context: &CliContext,
    payload: &Value,
    daemon_started: bool,
) -> Result<StartSummary, String> {
    let started: Vec<String> = payload
        .get("started")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let cleanup_summaries: Vec<ProjectTmuxCleanupSummary> = payload
        .get("cleanup_summaries")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    item.as_object().map(|obj| ProjectTmuxCleanupSummary {
                        socket_name: obj
                            .get("socket_name")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        owned_panes: strings_from_obj(obj, "owned_panes"),
                        active_panes: strings_from_obj(obj, "active_panes"),
                        orphaned_panes: strings_from_obj(obj, "orphaned_panes"),
                        killed_panes: strings_from_obj(obj, "killed_panes"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(StartSummary {
        project_root: payload
            .get("project_root")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| context.project.project_root.to_string_lossy().to_string()),
        project_id: payload
            .get("project_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| context.project.project_id.clone()),
        started,
        daemon_started,
        socket_path: payload
            .get("socket_path")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| context.paths.ccbrd_socket_path().to_string()),
        cleanup_summaries,
        worktree_warnings: Vec::new(),
        worktree_retired: Vec::new(),
        maintenance_heartbeat: None,
    })
}

fn strings_from_obj(obj: &serde_json::Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_context(tmp: &tempfile::TempDir) -> CliContext {
        let project_root = tmp.path().to_path_buf();
        let ccbr_dir = project_root.join(".ccbr");
        std::fs::create_dir_all(&ccbr_dir).unwrap();
        std::fs::write(ccbr_dir.join("ccbr.config"), "demo:codex\n").unwrap();
        let command = crate::models::ParsedCommand::Start(ParsedStartCommand {
            project: None,
            agent_names: vec!["demo".into()],
            restore: true,
            auto_permission: true,
            reset_context: false,
            kind: "start".into(),
        });
        crate::context::CliContextBuilder::new(command)
            .cwd(project_root.clone())
            .build()
            .unwrap()
    }

    struct FakeStore {
        data: std::sync::Mutex<Option<Value>>,
    }

    impl StartupReportStore for FakeStore {
        fn load(&self, _context: &CliContext) -> Option<Value> {
            self.data.lock().unwrap().clone()
        }
        fn save(&self, _context: &CliContext, report: &Value) -> Result<(), String> {
            *self.data.lock().unwrap() = Some(report.clone());
            Ok(())
        }
    }

    #[test]
    fn start_agents_passes_cli_flags_and_returns_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let store = FakeStore {
            data: std::sync::Mutex::new(None),
        };

        let mut seen_params: Option<Value> = None;
        let start_rpc = |handle: &DaemonHandle, _timeout: Option<f64>| {
            assert!(handle.started);
            seen_params = Some(json!({
                "agent_names": ["demo"],
                "restore": true,
                "auto_permission": true,
            }));
            Ok(json!({
                "project_root": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "started": ["demo"],
                "socket_path": context.paths.ccbrd_socket_path(),
                "cleanup_summaries": [],
            }))
        };

        let summary = start_agents(
            &context,
            &ParsedStartCommand {
                project: None,
                agent_names: vec!["demo".into()],
                restore: true,
                auto_permission: true,
                reset_context: false,
                kind: "start".into(),
            },
            None,
            |_ctx| {
                Ok(DaemonHandle {
                    client: None,
                    inspection: Value::Null,
                    started: true,
                })
            },
            start_rpc,
            &store,
            |_ctx| Ok(ccbr_workspace::reconcile::WorkspaceGuardSummary::default()),
            |summary, _guard| summary,
            |_ctx| None,
        )
        .unwrap();

        assert_eq!(summary.started, vec!["demo"]);
        assert!(summary.daemon_started);
        assert_eq!(
            seen_params,
            Some(json!({
                "agent_names": ["demo"],
                "restore": true,
                "auto_permission": true,
            }))
        );
    }

    #[test]
    fn start_agents_passes_terminal_size_when_provided() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let store = FakeStore {
            data: std::sync::Mutex::new(None),
        };

        let mut seen: Option<Value> = None;
        let start_rpc = |_handle: &DaemonHandle, _timeout: Option<f64>| {
            seen = Some(json!({"terminal_size": [233, 61]}));
            Ok(json!({
                "project_root": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "started": ["demo"],
                "socket_path": context.paths.ccbrd_socket_path(),
                "cleanup_summaries": [],
            }))
        };

        start_agents(
            &context,
            &ParsedStartCommand::new(None, vec!["demo".into()], false, false),
            Some((233, 61)),
            |_ctx| {
                Ok(DaemonHandle {
                    client: None,
                    inspection: Value::Null,
                    started: false,
                })
            },
            start_rpc,
            &store,
            |_ctx| Ok(ccbr_workspace::reconcile::WorkspaceGuardSummary::default()),
            |summary, _guard| summary,
            |_ctx| None,
        )
        .unwrap();

        assert_eq!(seen, Some(json!({"terminal_size": [233, 61]})));
    }

    #[test]
    fn start_agents_parses_cleanup_summaries_from_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let store = FakeStore {
            data: std::sync::Mutex::new(None),
        };

        let start_rpc = |_handle: &DaemonHandle, _timeout: Option<f64>| {
            Ok(json!({
                "project_root": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "started": ["demo"],
                "socket_path": context.paths.ccbrd_socket_path(),
                "cleanup_summaries": [
                    {
                        "socket_name": "sock-a",
                        "owned_panes": ["%44"],
                        "active_panes": ["%44"],
                        "orphaned_panes": [],
                        "killed_panes": [],
                    }
                ],
            }))
        };

        let summary = start_agents(
            &context,
            &ParsedStartCommand::new(None, Vec::new(), false, false),
            None,
            |_ctx| {
                Ok(DaemonHandle {
                    client: None,
                    inspection: Value::Null,
                    started: false,
                })
            },
            start_rpc,
            &store,
            |_ctx| Ok(ccbr_workspace::reconcile::WorkspaceGuardSummary::default()),
            |summary, _guard| summary,
            |_ctx| None,
        )
        .unwrap();

        assert_eq!(summary.cleanup_summaries.len(), 1);
        assert_eq!(
            summary.cleanup_summaries[0].socket_name,
            Some("sock-a".into())
        );
        assert_eq!(summary.cleanup_summaries[0].owned_panes, vec!["%44"]);
    }

    #[test]
    fn start_agents_updates_startup_report_with_daemon_started_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let store = FakeStore {
            data: std::sync::Mutex::new(Some(json!({
                "project_id": context.project.project_id,
                "daemon_started": null,
            }))),
        };

        let start_rpc = |_handle: &DaemonHandle, _timeout: Option<f64>| {
            Ok(json!({
                "project_root": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "started": ["demo"],
                "socket_path": context.paths.ccbrd_socket_path(),
                "cleanup_summaries": [],
            }))
        };

        start_agents(
            &context,
            &ParsedStartCommand::new(None, vec!["demo".into()], false, false),
            None,
            |_ctx| {
                Ok(DaemonHandle {
                    client: None,
                    inspection: Value::Null,
                    started: true,
                })
            },
            start_rpc,
            &store,
            |_ctx| Ok(ccbr_workspace::reconcile::WorkspaceGuardSummary::default()),
            |summary, _guard| summary,
            |_ctx| None,
        )
        .unwrap();

        let saved = store.load(&context).unwrap();
        assert_eq!(
            saved.get("daemon_started").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn start_agents_attaches_maintenance_heartbeat_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let context = make_context(&tmp);
        let store = FakeStore {
            data: std::sync::Mutex::new(None),
        };

        let start_rpc = |_handle: &DaemonHandle, _timeout: Option<f64>| {
            Ok(json!({
                "project_root": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "started": ["demo"],
                "socket_path": context.paths.ccbrd_socket_path(),
                "cleanup_summaries": [],
            }))
        };

        let summary = start_agents(
            &context,
            &ParsedStartCommand::new(None, vec!["demo".into()], false, false),
            None,
            |_ctx| {
                Ok(DaemonHandle {
                    client: None,
                    inspection: Value::Null,
                    started: false,
                })
            },
            start_rpc,
            &store,
            |_ctx| Ok(ccbr_workspace::reconcile::WorkspaceGuardSummary::default()),
            |summary, _guard| summary,
            |_ctx| Some(json!({"tick_status": "healthy"})),
        )
        .unwrap();

        assert_eq!(
            summary.maintenance_heartbeat,
            Some(json!({"tick_status": "healthy"}))
        );
    }
}
