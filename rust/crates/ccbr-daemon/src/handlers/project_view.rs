use std::collections::{HashMap, HashSet};

use ccbr_agents::models::{ProjectConfig, SidebarViewSpec};
use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::models::api_models::common::JobStatus;
use crate::models::api_models::records::JobRecord;
use crate::services::registry::AgentRuntimeEntry;

const PROJECT_VIEW_SCHEMA_VERSION: u64 = 1;
const PROJECT_VIEW_TTL_MS: u64 = 1000;
const PROJECT_VIEW_COMMS_LIMIT: usize = 8;

pub fn handle_project_view(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let schema_version = payload
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    if schema_version != PROJECT_VIEW_SCHEMA_VERSION {
        return Err(format!(
            "project_view schema_version must be {PROJECT_VIEW_SCHEMA_VERSION}"
        ));
    }

    let ns = app.project_namespace.load().cloned();
    let config = app.current_config.as_ref();
    let entry_window = entry_window(config, ns.as_ref());
    let active_pane_id = ns.as_ref().and_then(|n| n.active_panes.first().cloned());
    let active_window = active_window(ns.as_ref(), &entry_window, active_pane_id.as_deref());
    let windows = window_views(config, ns.as_ref(), &active_window);
    let agent_window = agent_window_map(&windows);
    let agent_names = agent_order(config, ns.as_ref(), &app.registry);
    let configured_agents: HashSet<String> = agent_names.iter().cloned().collect();
    let recoverability = app
        .dispatcher
        .comms_recoverability_view()
        .into_iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            Some((id, item))
        })
        .collect::<HashMap<_, _>>();

    let namespace_mounted = ns.is_some();
    let agents: Vec<Value> = agent_names
        .iter()
        .enumerate()
        .filter_map(|(order, agent_name)| {
            let runtime = app.registry.get(agent_name);
            if runtime.is_some_and(|entry| entry.state == "stopped") {
                return None;
            }
            Some(agent_view(
                app,
                config,
                agent_name,
                order,
                agent_window.get(agent_name).cloned().unwrap_or_default(),
                runtime,
                active_pane_id.as_deref(),
            ))
        })
        .collect();

    // Python wire shape: build_response returns {view, cache, schema_version}
    // with agents/windows/comms nested under `view`. The sidebar reads
    // response["view"]["agents"] etc., so the view payload must be wrapped.
    let mut comms_jobs: Vec<&JobRecord> = app
        .dispatcher
        .job_store
        .iter()
        .filter(|job| {
            let message_type = job.request.message_type.trim();
            message_type.is_empty() || message_type == "ask"
        })
        .collect();
    comms_jobs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    let comms: Vec<Value> = comms_jobs
        .into_iter()
        .take(PROJECT_VIEW_COMMS_LIMIT)
        .map(|job| comm_record(job, &configured_agents, recoverability.get(&job.job_id)))
        .collect();

    let generated_at = chrono::Utc::now().to_rfc3339();
    Ok(json!({
        "schema_version": schema_version,
        "view": {
            "schema_version": PROJECT_VIEW_SCHEMA_VERSION,
            "generated_at": generated_at,
            "project": {
                "id": app.project_id(),
                "root": app.project_root,
                "display_name": app.project_root.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            },
            "ccbd": {
                "state": if namespace_mounted { "mounted" } else { "unmounted" },
                "health": if namespace_mounted { "healthy" } else { "unmounted" },
                "generation": ns.as_ref().map(|n| n.namespace_epoch),
                "last_heartbeat_at": null,
            },
            "namespace": {
                "epoch": ns.as_ref().map(|n| n.namespace_epoch),
                "socket_path": ns.as_ref().map(|n| n.tmux_socket_path.clone()),
                "session_name": ns.as_ref().map(|n| n.tmux_session_name.clone()),
                "active_window": active_window,
                "active_pane_id": active_pane_id,
                "entry_window": entry_window,
                "sidebar": {
                    "view": sidebar_view(config),
                },
                "mounted": namespace_mounted,
                "project_root": app.project_root,
                "project_id": app.project_id(),
                "project_slug": app.layout.project_slug(),
                "daemon_status": if app.is_shutdown_requested() { "stopping" } else { "running" },
            },
            "windows": windows,
            "agents": agents,
            "comms": comms,
        },
        "cache": {
            "generated_at": generated_at,
            "ttl_ms": PROJECT_VIEW_TTL_MS,
            "sequence": 1,
        },
    }))
}

pub fn handle_project_view_dismiss_comms(
    _app: &mut CcbdApp,
    payload: &Value,
) -> Result<Value, String> {
    let comms_id = payload
        .get("id")
        .or_else(|| payload.get("comms_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    Ok(json!({
        "status": "dismissed",
        "id": comms_id,
        "dismissed_count": 0,
    }))
}

fn entry_window(
    config: Option<&ProjectConfig>,
    ns: Option<&crate::services::project_namespace::ProjectNamespace>,
) -> String {
    config
        .and_then(|c| c.entry_window.clone())
        .or_else(|| {
            config
                .and_then(|c| c.windows.as_ref())
                .and_then(|windows| windows.first())
                .map(|w| w.name.clone())
        })
        .or_else(|| ns.and_then(|n| n.windows.first()).map(|w| w.name.clone()))
        .unwrap_or_else(|| "ccbr".to_string())
}

fn active_window(
    ns: Option<&crate::services::project_namespace::ProjectNamespace>,
    entry_window: &str,
    active_pane_id: Option<&str>,
) -> String {
    let Some(ns) = ns else {
        return entry_window.to_string();
    };
    let Some(active_pane_id) = active_pane_id else {
        return entry_window.to_string();
    };
    let active_agent = ns
        .agent_panes
        .iter()
        .find(|(_, pane)| pane.as_str() == active_pane_id)
        .map(|(agent, _)| agent.as_str());
    if let Some(agent) = active_agent {
        if let Some(window) = ns
            .windows
            .iter()
            .find(|window| window.agents.iter().any(|name| name == agent))
        {
            return window.name.clone();
        }
    }
    entry_window.to_string()
}

fn window_views(
    config: Option<&ProjectConfig>,
    ns: Option<&crate::services::project_namespace::ProjectNamespace>,
    active_window: &str,
) -> Vec<Value> {
    let mut rows = Vec::new();
    if let Some(windows) = config.and_then(|c| c.windows.as_ref()) {
        for window in windows {
            rows.push(json!({
                "name": window.name,
                "label": window.name,
                "kind": "agents",
                "order": window.order,
                "tmux_window_id": ns.and_then(|n| n.windows.iter().find(|w| w.name == window.name)).and_then(|w| w.window_id.clone()),
                "tmux_window_index": null,
                "window_id": ns.and_then(|n| n.windows.iter().find(|w| w.name == window.name)).and_then(|w| w.window_id.clone()),
                "active": window.name == active_window,
                "sidebar_pane_id": null,
                "agents": window.agent_names,
            }));
        }
    } else if let Some(ns) = ns {
        for (order, window) in ns.windows.iter().enumerate() {
            rows.push(json!({
                "name": window.name,
                "label": window.name,
                "kind": "agents",
                "order": order,
                "tmux_window_id": window.window_id,
                "tmux_window_index": null,
                "window_id": window.window_id,
                "active": window.name == active_window,
                "sidebar_pane_id": null,
                "agents": window.agents,
            }));
        }
    }
    if let Some(tool_windows) = config.and_then(|c| c.tool_windows.as_ref()) {
        let offset = rows.len() as u32;
        for tool in tool_windows {
            rows.push(json!({
                "name": tool.name,
                "label": tool.label.clone().unwrap_or_else(|| tool.name.clone()),
                "kind": "tool",
                "show_in_sidebar": tool.show_in_sidebar,
                "order": offset + tool.order,
                "tmux_window_id": ns.and_then(|n| n.windows.iter().find(|w| w.name == tool.name)).and_then(|w| w.window_id.clone()),
                "tmux_window_index": null,
                "window_id": ns.and_then(|n| n.windows.iter().find(|w| w.name == tool.name)).and_then(|w| w.window_id.clone()),
                "active": tool.name == active_window,
                "sidebar_pane_id": null,
                "agents": [],
            }));
        }
    }
    rows
}

fn agent_window_map(windows: &[Value]) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for window in windows {
        let window_name = window
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        for agent in window
            .get("agents")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
        {
            result.insert(agent.to_string(), window_name.clone());
        }
    }
    result
}

fn agent_order(
    config: Option<&ProjectConfig>,
    ns: Option<&crate::services::project_namespace::ProjectNamespace>,
    registry: &crate::services::registry::AgentRegistry,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(windows) = config.and_then(|c| c.windows.as_ref()) {
        for window in windows {
            for agent in &window.agent_names {
                push_unique(&mut names, agent);
            }
        }
    } else if let Some(config) = config {
        if !config.default_agents.is_empty() {
            for agent in &config.default_agents {
                push_unique(&mut names, agent);
            }
        } else {
            let mut keys = config.agents.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for agent in keys {
                push_unique(&mut names, &agent);
            }
        }
    } else if let Some(ns) = ns {
        for agent in &ns.agent_names {
            push_unique(&mut names, agent);
        }
    }
    let mut runtime_names = registry
        .all_entries()
        .iter()
        .map(|entry| entry.agent_name.clone())
        .collect::<Vec<_>>();
    runtime_names.sort();
    for agent in runtime_names {
        push_unique(&mut names, &agent);
    }
    names
}

fn push_unique(names: &mut Vec<String>, value: &str) {
    if !value.trim().is_empty() && !names.iter().any(|name| name == value) {
        names.push(value.to_string());
    }
}

fn agent_view(
    app: &CcbdApp,
    config: Option<&ProjectConfig>,
    agent_name: &str,
    order: usize,
    window_name: String,
    runtime: Option<&AgentRuntimeEntry>,
    active_pane_id: Option<&str>,
) -> Value {
    let active_job_id = app.dispatcher.state.active_job(agent_name);
    let queued_job_id = app.dispatcher.state.next_queued(agent_name);
    let current_job = active_job_id
        .or(queued_job_id)
        .and_then(|job_id| app.dispatcher.get(job_id));
    let queue_depth =
        app.dispatcher.state.queue_depth(agent_name) + usize::from(active_job_id.is_some());
    let provider = runtime
        .map(|entry| entry.provider.clone())
        .or_else(|| {
            config
                .and_then(|c| c.agents.get(agent_name))
                .map(|spec| spec.provider.clone())
        })
        .unwrap_or_default();
    let runtime_state = runtime
        .map(|entry| entry.state.as_str())
        .unwrap_or("stopped");
    let runtime_health = runtime
        .map(|entry| entry.health.as_str())
        .unwrap_or("unknown");
    let pane_id = runtime.and_then(|entry| entry.pane_id.clone());
    let active = pane_id
        .as_deref()
        .is_some_and(|pane| Some(pane) == active_pane_id);
    let (activity_state, activity_source, activity_reason, activity_symbol, activity_color) =
        activity_fields(current_job, runtime_state);

    json!({
        "name": agent_name,
        "provider": provider,
        "window": window_name,
        "order": order,
        "pane_id": pane_id,
        "active": active,
        "queue_depth": queue_depth,
        "state": runtime_state,
        "health": runtime_health,
        "activity_state": activity_state,
        "activity_source": activity_source,
        "activity_reason": activity_reason,
        "activity_symbol": activity_symbol,
        "activity_color": activity_color,
        "current_job_id": current_job.map(|job| job.job_id.clone()),
        "last_progress_at": current_job.map(|job| job.updated_at.clone()),
        "callback_waiting_child_job_id": null,
        "callback_waiting_child_agent": null,
        "callback_waiting_state": null,
        "runtime_state": runtime_state,
        "runtime_health": runtime_health,
        "reconcile_state": null,
        "workspace_path": runtime.and_then(|entry| entry.workspace_path.clone()),
    })
}

fn activity_fields(
    job: Option<&JobRecord>,
    runtime_state: &str,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    if let Some(job) = job {
        return match job.status {
            JobStatus::Running => ("active", "ccb_job", "job_running", "●", "green"),
            JobStatus::Accepted | JobStatus::Queued => {
                ("pending", "ccb_job", "job_queued", "◌", "yellow")
            }
            JobStatus::Failed | JobStatus::Incomplete => {
                ("failed", "ccb_job", "job_failed", "×", "red")
            }
            JobStatus::Cancelled => ("idle", "ccb_job", "job_cancelled", "○", "blue"),
            JobStatus::Completed => ("idle", "ccb_job", "job_completed", "○", "blue"),
        };
    }
    if matches!(runtime_state, "busy" | "running" | "active") {
        ("active", "pane_liveness", "runtime_active", "●", "green")
    } else {
        ("idle", "pane_liveness", "runtime_idle", "○", "blue")
    }
}

fn sidebar_view(config: Option<&ProjectConfig>) -> Value {
    let view = config.and_then(|c| c.sidebar_view.as_ref());
    let default_view;
    let spec: &SidebarViewSpec = if let Some(view) = view {
        view
    } else {
        default_view = SidebarViewSpec::default();
        &default_view
    };
    spec.to_record()
}

fn comm_record(
    job: &JobRecord,
    configured_agents: &HashSet<String>,
    recoverability: Option<&Value>,
) -> Value {
    let (business_status, status_label) = comm_business_status(job, configured_agents);
    json!({
        "id": job.job_id,
        "short_id": short_id(&job.job_id),
        "created_at": job.created_at,
        "updated_at": job.updated_at,
        "sender": job.request.from_actor,
        "target": job.target_name,
        "from_actor": job.request.from_actor,
        "to_agent": job.agent_name,
        "message_type": job.request.message_type,
        "status": status_text(job.status),
        "business_status": business_status,
        "status_label": status_label,
        "body_preview": body_preview(&job.request.body),
        "reply_status": null,
        "reply_delivery_job_id": null,
        "callback": job.request.reply_to.is_some(),
        "short_reason": job.terminal_decision.as_ref().and_then(|v| v.get("reason")).and_then(|v| v.as_str()),
        "recoverable": recoverability.and_then(|v| v.get("recoverable")).and_then(|v| v.as_bool()).unwrap_or(false),
        "recover_target": recoverability.and_then(|v| v.get("recover_target")).cloned().unwrap_or(Value::Null),
        "block_reason": recoverability.and_then(|v| v.get("block_reason")).cloned().unwrap_or(Value::Null),
    })
}

fn comm_business_status(
    job: &JobRecord,
    configured_agents: &HashSet<String>,
) -> (&'static str, &'static str) {
    match job.status {
        JobStatus::Accepted | JobStatus::Queued => ("sending", "send"),
        JobStatus::Running => ("replying", "work"),
        JobStatus::Completed => {
            if job.request.silence_on_success {
                ("completed", "done")
            } else if configured_agents.contains(job.request.from_actor.as_str())
                && job.request.from_actor != job.target_name
            {
                ("delivering", "back")
            } else {
                ("replied", "done")
            }
        }
        JobStatus::Cancelled => ("cancelled", "fail"),
        JobStatus::Incomplete => ("incomplete", "fail"),
        JobStatus::Failed => ("failed", "fail"),
    }
}

fn status_text(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Accepted => "accepted",
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Cancelled => "cancelled",
        JobStatus::Failed => "failed",
        JobStatus::Incomplete => "incomplete",
    }
}

fn body_preview(value: &str) -> String {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= 48 {
        text
    } else {
        let prefix: String = text.chars().take(45).collect();
        format!("{prefix}...")
    }
}

fn short_id(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= 4 {
        value.to_string()
    } else {
        chars[chars.len() - 4..].iter().collect()
    }
}
