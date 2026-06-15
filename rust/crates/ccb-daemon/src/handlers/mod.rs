use serde_json::Value;
use std::collections::HashMap;

use crate::app::CcbdApp;

pub type HandlerFn = Box<dyn Fn(&mut CcbdApp, &Value) -> Result<Value, String> + Send + Sync>;

pub struct HandlerRegistry {
    handlers: HashMap<String, HandlerFn>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, op: &str, handler: HandlerFn) {
        self.handlers.insert(op.to_string(), handler);
    }

    pub fn dispatch(&self, op: &str, app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
        match self.handlers.get(op) {
            Some(handler) => handler(app, payload),
            None => Err(format!("unknown op: {}", op)),
        }
    }

    pub fn has_handler(&self, op: &str) -> bool {
        self.handlers.contains_key(op)
    }
}

pub mod ack;
pub mod ask;
pub mod attach;
pub mod cancel;
pub mod cleanup;
pub mod comms_recover;
pub mod fault;
pub mod get;
pub mod inbox;
pub mod logs;
pub mod mailbox_head;
pub mod maintenance_tick;
pub mod ping;
pub mod project_clear;
pub mod project_focus;
pub mod project_reload;
pub mod project_restart;
pub mod project_view;
pub mod queue;
pub mod restore;
pub mod resubmit;
pub mod retry;
pub mod shutdown;
pub mod start;
pub mod stop_all;
pub mod submit;
pub mod trace;
pub mod watch;

/// Build the default handler registry wired to daemon application state.
pub fn build_registry() -> HandlerRegistry {
    use crate::handlers::*;
    let mut reg = HandlerRegistry::new();

    reg.register("start", Box::new(start::handle_start));
    reg.register("ask", Box::new(ask::handle_ask));
    reg.register("shutdown", Box::new(shutdown::handle_shutdown));
    // TODO(phase2-protocol): Python v7.5.2 CLI `ask` uses daemon op `submit` and
    // relies on the dispatcher for async delivery. For now `submit` remains
    // enqueue-only to preserve test contracts; align delivery semantics in Phase 2.
    reg.register("submit", Box::new(submit::handle_submit));
    reg.register("cancel", Box::new(cancel::handle_cancel));
    reg.register("ping", Box::new(ping::handle_ping));
    reg.register("project_view", Box::new(project_view::handle_project_view));
    reg.register(
        "project_view_dismiss_comms",
        Box::new(project_view::handle_project_view_dismiss_comms),
    );
    reg.register("queue", Box::new(queue::handle_queue));
    reg.register("trace", Box::new(trace::handle_trace));
    reg.register("watch", Box::new(watch::handle_watch));
    reg.register("inbox", Box::new(inbox::handle_inbox));
    reg.register("ack", Box::new(ack::handle_ack));
    reg.register("resubmit", Box::new(resubmit::handle_resubmit));
    reg.register("retry", Box::new(retry::handle_retry));
    reg.register("stop-all", Box::new(stop_all::handle_stop_all));
    reg.register(
        "project_clear_context",
        Box::new(project_clear::handle_project_clear),
    );
    reg.register(
        "project_focus_window",
        Box::new(project_focus::handle_project_focus_window),
    );
    reg.register(
        "project_focus_agent",
        Box::new(project_focus::handle_project_focus_agent),
    );
    reg.register(
        "project_restart_agent",
        Box::new(project_restart::handle_project_restart_agent),
    );
    reg.register(
        "project_restart_panes",
        Box::new(project_restart::handle_project_restart_panes),
    );
    reg.register(
        "project_reload_config",
        Box::new(project_reload::handle_project_reload),
    );
    reg.register("get", Box::new(get::handle_get));
    reg.register("restore", Box::new(restore::handle_restore));
    reg.register("attach", Box::new(attach::handle_attach));
    reg.register(
        "comms_recover",
        Box::new(comms_recover::handle_comms_recover),
    );
    reg.register("mailbox_head", Box::new(mailbox_head::handle_mailbox_head));
    reg.register(
        "maintenance_tick",
        Box::new(maintenance_tick::handle_maintenance_tick),
    );
    reg.register("logs", Box::new(logs::handle_logs));
    reg.register("cleanup", Box::new(cleanup::handle_cleanup));
    reg.register("fault_list", Box::new(fault::handle_fault_list));
    reg.register("fault_arm", Box::new(fault::handle_fault_arm));
    reg.register("fault_clear", Box::new(fault::handle_fault_clear));

    reg
}

/// Helper to extract a bool field with a default value.
pub fn bool_field(payload: &Value, key: &str, default: bool) -> bool {
    payload
        .get(key)
        .map(|v| v.as_bool().unwrap_or(default))
        .unwrap_or(default)
}

/// Helper to extract a string field.
pub fn str_field(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
pub mod ping_runtime;
pub mod project_reload_cache;
pub mod project_reload_metrics;
pub mod project_reload_payload;
