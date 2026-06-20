//! Mirrors Python `lib/ccbd/socket_client.py`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::api_models::{MessageEnvelope, RpcRequest};
use crate::socket_client_runtime::endpoints;
use crate::socket_client_runtime::errors::CcbdClientError;
use crate::socket_client_runtime::transport;

/// RPC client for the local CCB daemon.
#[derive(Debug, Clone)]
pub struct CcbdClient {
    socket_path: PathBuf,
    timeout_s: f64,
}

impl CcbdClient {
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            timeout_s: resolve_timeout(None),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn timeout_s(&self) -> f64 {
        self.timeout_s
    }

    pub fn with_timeout(&self, timeout_s: f64) -> Self {
        Self {
            socket_path: self.socket_path.clone(),
            timeout_s: resolve_timeout(Some(timeout_s)),
        }
    }

    /// Send an RPC request and return the payload on success.
    pub fn request(&self, op: &str, payload: Option<Value>) -> Result<Value, CcbdClientError> {
        let req = RpcRequest {
            op: op.to_string(),
            request: payload.unwrap_or_else(|| Value::Object(Default::default())),
        };
        let mut sock = transport::connect_socket_unix(&self.socket_path, self.timeout_s)?;
        transport::send_request(&mut sock, &req)?;
        let raw = transport::recv_response_line(&mut sock)?;
        if raw.is_empty() {
            return Err(CcbdClientError::new("empty response from ccbd"));
        }
        let response = transport::decode_response(&raw)?;
        if !response.ok {
            return Err(CcbdClientError::new(
                response
                    .error
                    .unwrap_or_else(|| "ccbd request failed".into()),
            ));
        }
        Ok(response.payload.unwrap_or(Value::Null))
    }

    pub fn submit(&self, request: &dyn MessageEnvelope) -> Result<Value, CcbdClientError> {
        self.request("submit", Some(endpoints::payload_submit(request)))
    }

    pub fn get(&self, job_id: &str) -> Result<Value, CcbdClientError> {
        self.request("get", Some(endpoints::payload_get(job_id)))
    }

    pub fn watch(&self, target: &str, cursor: i64) -> Result<Value, CcbdClientError> {
        self.request("watch", Some(endpoints::payload_watch(target, cursor)))
    }

    pub fn queue(&self, target: &str, detail: Option<bool>) -> Result<Value, CcbdClientError> {
        self.request("queue", Some(endpoints::payload_queue(target, detail)))
    }

    pub fn trace(&self, target: &str) -> Result<Value, CcbdClientError> {
        self.request("trace", Some(endpoints::payload_trace(target)))
    }

    pub fn resubmit(&self, message_id: &str) -> Result<Value, CcbdClientError> {
        self.request("resubmit", Some(endpoints::payload_resubmit(message_id)))
    }

    pub fn retry(&self, target: &str) -> Result<Value, CcbdClientError> {
        self.request("retry", Some(endpoints::payload_retry(target)))
    }

    pub fn comms_recover(
        &self,
        job_id: &str,
        reply_delivery_job_id: Option<&str>,
        block_reason: Option<&str>,
    ) -> Result<Value, CcbdClientError> {
        self.request(
            "comms_recover",
            Some(endpoints::payload_comms_recover(
                job_id,
                reply_delivery_job_id,
                block_reason,
            )),
        )
    }

    pub fn inbox(&self, agent_name: &str, detail: Option<bool>) -> Result<Value, CcbdClientError> {
        self.request("inbox", Some(endpoints::payload_inbox(agent_name, detail)))
    }

    pub fn mailbox_head(&self, agent_name: &str) -> Result<Value, CcbdClientError> {
        self.request(
            "mailbox_head",
            Some(endpoints::payload_mailbox_head(agent_name)),
        )
    }

    pub fn ack(
        &self,
        agent_name: &str,
        inbound_event_id: Option<&str>,
    ) -> Result<Value, CcbdClientError> {
        self.request(
            "ack",
            Some(endpoints::payload_ack(agent_name, inbound_event_id)),
        )
    }

    pub fn cancel(&self, job_id: &str) -> Result<Value, CcbdClientError> {
        self.request("cancel", Some(endpoints::payload_cancel(job_id)))
    }

    pub fn start(
        &self,
        agent_names: &[String],
        restore: bool,
        auto_permission: bool,
        terminal_size: Option<(u32, u32)>,
    ) -> Result<Value, CcbdClientError> {
        self.request(
            "start",
            Some(endpoints::payload_start(
                agent_names,
                restore,
                auto_permission,
                terminal_size,
            )),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn attach(
        &self,
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
    ) -> Result<Value, CcbdClientError> {
        self.request(
            "attach",
            Some(endpoints::payload_attach(
                agent_name,
                workspace_path,
                backend_type,
                pid,
                runtime_ref,
                session_ref,
                health,
                provider,
                runtime_root,
                runtime_pid,
                terminal_backend,
                pane_id,
                active_pane_id,
                pane_title_marker,
                pane_state,
                tmux_socket_name,
                tmux_window_name,
                tmux_window_id,
                session_file,
                session_id,
                lifecycle_state,
                managed_by,
                binding_source,
            )),
        )
    }

    pub fn restore(&self, agent_name: &str) -> Result<Value, CcbdClientError> {
        self.request("restore", Some(endpoints::payload_restore(agent_name)))
    }

    pub fn ping(&self, target: &str) -> Result<Value, CcbdClientError> {
        self.request("ping", Some(endpoints::payload_ping(target)))
    }

    pub fn shutdown(&self) -> Result<Value, CcbdClientError> {
        self.request("shutdown", Some(endpoints::payload_shutdown()))
    }

    pub fn stop_all(&self, force: bool) -> Result<Value, CcbdClientError> {
        self.request("stop_all", Some(endpoints::payload_stop_all(force)))
    }

    pub fn project_view(&self, schema_version: i64) -> Result<Value, CcbdClientError> {
        self.request(
            "project_view",
            Some(endpoints::payload_project_view(schema_version)),
        )
    }

    pub fn project_view_dismiss_comms(&self, comms_id: &str) -> Result<Value, CcbdClientError> {
        self.request(
            "project_view_dismiss_comms",
            Some(endpoints::payload_project_view_dismiss_comms(comms_id)),
        )
    }

    pub fn project_restart_panes(&self) -> Result<Value, CcbdClientError> {
        self.request(
            "project_restart_panes",
            Some(endpoints::payload_project_restart_panes()),
        )
    }

    pub fn project_restart_agent(&self, agent_name: &str) -> Result<Value, CcbdClientError> {
        self.request(
            "project_restart_agent",
            Some(endpoints::payload_project_restart_agent(agent_name)),
        )
    }

    pub fn project_clear_context(&self, agent_names: &[String]) -> Result<Value, CcbdClientError> {
        self.request(
            "project_clear_context",
            Some(endpoints::payload_project_clear_context(agent_names)),
        )
    }

    pub fn project_reload_config(&self, dry_run: bool) -> Result<Value, CcbdClientError> {
        self.request(
            "project_reload_config",
            Some(endpoints::payload_project_reload_config(dry_run)),
        )
    }

    pub fn project_focus_window(
        &self,
        window: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<Value, CcbdClientError> {
        self.request(
            "project_focus_window",
            Some(endpoints::payload_project_focus_window(
                window,
                namespace_epoch,
            )),
        )
    }

    pub fn project_focus_agent(
        &self,
        agent: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<Value, CcbdClientError> {
        self.request(
            "project_focus_agent",
            Some(endpoints::payload_project_focus_agent(
                agent,
                namespace_epoch,
            )),
        )
    }
}

fn resolve_timeout(explicit: Option<f64>) -> f64 {
    if let Some(t) = explicit {
        if t.is_finite() && t >= 0.1 {
            return t;
        }
        return 3.0;
    }
    if let Ok(raw) = std::env::var("CCB_CCBD_CLIENT_TIMEOUT_S") {
        if let Ok(t) = raw.parse::<f64>() {
            if t.is_finite() && t >= 0.1 {
                return t;
            }
        }
    }
    3.0
}
