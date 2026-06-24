//! Mirrors Python `lib/ccbrd/socket_client_runtime/endpoints.py`.

use serde_json::{json, Value};

use crate::api_models::MessageEnvelope;

/// Resolve the daemon operation name for a dynamic client endpoint.
pub fn endpoint_op(name: &str) -> Option<&'static str> {
    match name {
        "submit" => Some("submit"),
        "get" => Some("get"),
        "watch" => Some("watch"),
        "queue" => Some("queue"),
        "trace" => Some("trace"),
        "resubmit" => Some("resubmit"),
        "retry" => Some("retry"),
        "comms_recover" => Some("comms_recover"),
        "inbox" => Some("inbox"),
        "mailbox_head" => Some("mailbox_head"),
        "ack" => Some("ack"),
        "cancel" => Some("cancel"),
        "start" => Some("start"),
        "attach" => Some("attach"),
        "restore" => Some("restore"),
        "ping" => Some("ping"),
        "shutdown" => Some("shutdown"),
        "stop_all" => Some("stop-all"),
        "project_view" => Some("project_view"),
        "project_view_dismiss_comms" => Some("project_view_dismiss_comms"),
        "project_restart_panes" => Some("project_restart_panes"),
        "project_restart_agent" => Some("project_restart_agent"),
        "project_clear_context" => Some("project_clear_context"),
        "project_reload_config" => Some("project_reload_config"),
        "project_focus_window" => Some("project_focus_window"),
        "project_focus_agent" => Some("project_focus_agent"),
        _ => None,
    }
}

pub fn payload_submit(request: &dyn MessageEnvelope) -> Value {
    request.to_record()
}

pub fn payload_get(job_id: &str) -> Value {
    json!({"job_id": job_id})
}

pub fn payload_watch(target: &str, cursor: i64) -> Value {
    json!({"target": target, "cursor": cursor})
}

pub fn payload_queue(target: &str, detail: Option<bool>) -> Value {
    let mut payload = json!({"target": target});
    if let Some(d) = detail {
        payload["detail"] = json!(d);
    }
    payload
}

pub fn payload_trace(target: &str) -> Value {
    json!({"target": target})
}

pub fn payload_resubmit(message_id: &str) -> Value {
    json!({"message_id": message_id})
}

pub fn payload_retry(target: &str) -> Value {
    json!({"target": target})
}

#[allow(clippy::too_many_arguments)]
pub fn payload_comms_recover(
    job_id: &str,
    reply_delivery_job_id: Option<&str>,
    block_reason: Option<&str>,
) -> Value {
    let mut payload = json!({"job_id": job_id});
    if let Some(v) = reply_delivery_job_id {
        payload["reply_delivery_job_id"] = json!(v);
    }
    if let Some(v) = block_reason {
        payload["block_reason"] = json!(v);
    }
    payload
}

pub fn payload_inbox(agent_name: &str, detail: Option<bool>) -> Value {
    let mut payload = json!({"agent_name": agent_name});
    if let Some(d) = detail {
        payload["detail"] = json!(d);
    }
    payload
}

pub fn payload_mailbox_head(agent_name: &str) -> Value {
    json!({"agent_name": agent_name})
}

pub fn payload_ack(agent_name: &str, inbound_event_id: Option<&str>) -> Value {
    let mut payload = json!({"agent_name": agent_name});
    if let Some(v) = inbound_event_id {
        payload["inbound_event_id"] = json!(v);
    }
    payload
}

pub fn payload_cancel(job_id: &str) -> Value {
    json!({"job_id": job_id})
}

pub fn payload_start(
    agent_names: &[String],
    restore: bool,
    auto_permission: bool,
    terminal_size: Option<(u32, u32)>,
) -> Value {
    let mut payload = json!({
        "agent_names": agent_names,
        "restore": restore,
        "auto_permission": auto_permission,
    });
    if let Some((w, h)) = terminal_size {
        payload["terminal_width"] = json!(w);
        payload["terminal_height"] = json!(h);
    }
    payload
}

#[allow(clippy::too_many_arguments)]
pub fn payload_attach(
    agent_name: &str,
    workspace_path: &str,
    backend_type: &str,
    pid: Option<i64>,
    runtime_ref: Option<&str>,
    session_ref: Option<&str>,
    health: Option<&str>,
    provider: Option<&str>,
    runtime_root: Option<&str>,
    runtime_pid: Option<i64>,
    terminal_backend: Option<&str>,
    pane_id: Option<&str>,
    active_pane_id: Option<&str>,
    pane_title_marker: Option<&str>,
    pane_state: Option<&str>,
    tmux_socket_name: Option<&str>,
    tmux_window_name: Option<&str>,
    tmux_window_id: Option<&str>,
    session_file: Option<&str>,
    session_id: Option<&str>,
    lifecycle_state: Option<&str>,
    managed_by: Option<&str>,
    binding_source: Option<&str>,
) -> Value {
    json!({
        "agent_name": agent_name,
        "workspace_path": workspace_path,
        "backend_type": backend_type,
        "pid": pid,
        "runtime_ref": runtime_ref,
        "session_ref": session_ref,
        "health": health,
        "provider": provider,
        "runtime_root": runtime_root,
        "runtime_pid": runtime_pid,
        "terminal_backend": terminal_backend,
        "pane_id": pane_id,
        "active_pane_id": active_pane_id,
        "pane_title_marker": pane_title_marker,
        "pane_state": pane_state,
        "tmux_socket_name": tmux_socket_name,
        "tmux_window_name": tmux_window_name,
        "tmux_window_id": tmux_window_id,
        "session_file": session_file,
        "session_id": session_id,
        "lifecycle_state": lifecycle_state,
        "managed_by": managed_by,
        "binding_source": binding_source,
    })
}

pub fn payload_restore(agent_name: &str) -> Value {
    json!({"agent_name": agent_name})
}

pub fn payload_ping(target: &str) -> Value {
    json!({"target": target})
}

pub fn payload_shutdown() -> Value {
    json!({})
}

pub fn payload_stop_all(force: bool) -> Value {
    json!({"force": force})
}

pub fn payload_project_view(schema_version: i64) -> Value {
    json!({"schema_version": schema_version})
}

pub fn payload_project_view_dismiss_comms(comms_id: &str) -> Value {
    json!({"id": comms_id})
}

pub fn payload_project_restart_panes() -> Value {
    json!({})
}

pub fn payload_project_restart_agent(agent_name: &str) -> Value {
    json!({"agent_name": agent_name})
}

pub fn payload_project_clear_context(agent_names: &[String]) -> Value {
    json!({"agent_names": agent_names})
}

pub fn payload_project_reload_config(dry_run: bool) -> Value {
    json!({"dry_run": dry_run})
}

pub fn payload_project_focus_window(window: &str, namespace_epoch: Option<i64>) -> Value {
    let mut payload = json!({"window": window});
    if let Some(v) = namespace_epoch {
        payload["namespace_epoch"] = json!(v);
    }
    payload
}

pub fn payload_project_focus_agent(agent: &str, namespace_epoch: Option<i64>) -> Value {
    let mut payload = json!({"agent": agent});
    if let Some(v) = namespace_epoch {
        payload["namespace_epoch"] = json!(v);
    }
    payload
}
