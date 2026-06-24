//! Mirrors Python `lib/cli/phase2_services.py`.
//!
//! Provides a concrete `Phase2Services` implementation backed by the daemon
//! socket client (`crate::ccbrd::CcbdClient`). Most service methods forward to a
//! matching daemon RPC; a few read local state where the Rust implementation is
//! already complete (e.g. `ps_summary`).

use std::io::Write;

use serde_json::{json, Value};

use crate::ccbrd::CcbdClient;
use crate::context::CliContext;
use crate::models::ParsedPsCommand;
use crate::phase2_runtime::handlers_ops::Phase2Services;
use crate::render_runtime::common::write_lines;
use crate::services::diagnostics_runtime::bundle::export_diagnostic_bundle;
use crate::services::socket_path_for_project;

/// `Phase2Services` implementation that issues RPCs to the local CCBR daemon.
#[derive(Debug, Clone)]
pub struct DaemonPhase2Services {
    client: CcbdClient,
}

impl DaemonPhase2Services {
    /// Build a service bundle for the project in `context`.
    pub fn from_context(context: &CliContext) -> Self {
        let socket_path = socket_path_for_project(context.paths.project_root.as_std_path());
        Self {
            client: CcbdClient::new(&socket_path),
        }
    }

    /// Build a service bundle from an existing daemon client (useful in tests).
    pub fn with_client(client: CcbdClient) -> Self {
        Self { client }
    }

    fn rpc(&self, op: &str, payload: &Value) -> Value {
        match self.client.request(op, payload) {
            Ok(value) => value,
            Err(err) => json!({
                "error": err.to_string(),
                "status": "error",
            }),
        }
    }
}

impl Phase2Services for DaemonPhase2Services {
    fn write_lines<W: Write>(&self, out: &mut W, lines: &[String]) {
        write_lines(out, lines);
    }

    fn kill_project(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("stop-all", command)
    }

    fn cleanup_project_storage(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("cleanup", command)
    }

    fn clear_agent_context(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("project_clear_context", command)
    }

    fn agent_logs(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("logs", command)
    }

    fn maintenance_status(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("maintenance_tick", command)
    }

    fn ps_summary(&self, context: &CliContext, _command: &Value) -> Value {
        crate::services::ps::ps_summary(context, &ParsedPsCommand::new(None))
    }

    fn export_diagnostic_bundle(&self, context: &CliContext, command: &Value) -> Value {
        match export_diagnostic_bundle(context, command) {
            Ok(summary) => serde_json::to_value(summary).unwrap_or_else(|e| {
                json!({
                    "status": "error",
                    "error": format!("failed to serialize bundle summary: {e}"),
                })
            }),
            Err(err) => json!({
                "status": "error",
                "error": err.to_string(),
            }),
        }
    }

    fn doctor_storage_summary(&self, context: &CliContext) -> Value {
        match ccbr_storage_classification::classification::summarize_storage(&context.paths) {
            Ok(summary) => {
                let mut payload = Value::Object(summary);
                if let Value::Object(map) = &mut payload {
                    map.insert(
                        "project".to_string(),
                        json!(context.project.project_root.to_string_lossy().to_string()),
                    );
                    map.insert("project_id".to_string(), json!(context.project.project_id));
                    map.insert("status".to_string(), json!("ok"));
                }
                payload
            }
            Err(err) => json!({
                "status": "error",
                "error": err.to_string(),
                "project": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
            }),
        }
    }

    fn doctor_summary(&self, context: &CliContext) -> Value {
        let ping = self.rpc("ping", &json!({"target": "ccbrd"}));
        let daemon_ok = ping.get("pong").and_then(|v| v.as_bool()).unwrap_or(false);
        let state = if daemon_ok {
            "reachable"
        } else {
            "unreachable"
        };
        json!({
            "project": context.project.project_root.to_string_lossy().to_string(),
            "project_id": context.project.project_id,
            "installation": {
                "path": "",
                "install_mode": "",
                "source_kind": "",
                "version": crate::entry::VERSION,
                "channel": "",
                "build_time": "",
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            },
            "runtime": {
                "user_id": "",
                "user_name": "",
                "home": "",
                "root_runtime": false,
                "install_root_owned": false,
                "install_user_id": "",
                "install_user_name": "",
                "sudo_user": "",
                "project_owner": "",
                "ccbr_dir_owner": "",
                "install_owner": "",
            },
            "requirements": {
                "python_executable": "",
                "python_version": "",
                "tmux_available": false,
                "tmux_path": "",
            },
            "ccbrd": {
                "state": state,
                "socket_path": context.paths.ccbrd_socket_path().to_string(),
                "pong": daemon_ok,
            },
            "status": if daemon_ok { "ok" } else { "degraded" },
        })
    }

    fn list_fault_rules(&self, _context: &CliContext) -> Value {
        self.rpc("fault_list", &json!({}))
    }

    fn arm_fault_rule(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("fault_arm", command)
    }

    fn clear_fault_rule(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("fault_clear", command)
    }

    fn reload_config(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("project_reload_config", command)
    }

    fn restart_agent(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("project_restart_agent", command)
    }

    fn submit_ask(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("submit", command)
    }

    fn ping_target(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("ping", command)
    }

    fn pend_target(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("get", command)
    }

    fn queue_target(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("queue", command)
    }

    fn trace_target(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("trace", command)
    }

    fn inbox_target(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("inbox", command)
    }

    fn ack_reply(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("ack", command)
    }

    fn watch_target(&self, _context: &CliContext, command: &Value) -> Vec<Value> {
        vec![self.rpc("watch", command)]
    }

    fn resubmit_message(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("resubmit", command)
    }

    fn retry_attempt(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("retry", command)
    }

    fn wait_for_replies(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("watch", command)
    }

    fn cancel_job(&self, _context: &CliContext, command: &Value) -> Value {
        self.rpc("cancel", command)
    }

    fn validate_config_context(&self, context: &CliContext) -> Value {
        match ccbr_agents::config::load_project_config(&context.paths) {
            Ok(result) => json!({
                "status": "ok",
                "project_root": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "source_path": result.source_path.map(|p| p.to_string()),
                "source_kind": result.source_kind,
                "used_builtin_default": result.used_default,
                "default_agents": result.config.default_agents,
                "agent_names": result.config.agents.keys().collect::<Vec<_>>(),
                "cmd_enabled": result.config.cmd_enabled,
                "layout_spec": result.config.layout_spec.unwrap_or_default(),
                "style_warnings": Vec::<String>::new(),
            }),
            Err(err) => json!({
                "status": "error",
                "error": err.to_string(),
            }),
        }
    }

    fn start_agents(
        &self,
        _context: &CliContext,
        command: &Value,
        _terminal_size: Option<(u16, u16)>,
    ) -> Value {
        self.rpc("start", command)
    }
}
