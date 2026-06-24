//! Mirrors Python `lib/cli/phase2_runtime/handlers_ops.py`.
//! 1:1 file alignment.

use serde_json::Value;
use std::io::Write;

use crate::render_runtime::ops_views::{
    render_cleanup, render_clear, render_doctor, render_doctor_bundle, render_doctor_storage,
    render_fault_arm, render_fault_clear, render_fault_list, render_kill, render_logs,
    render_maintenance, render_ps, render_reload, render_restart,
};
/// Trait mirroring the Python `services` object passed to phase2 handlers.
///
/// Service methods return JSON payloads; concrete implementations will be wired
/// once the daemon runtime is available.
pub trait Phase2Services {
    /// Write lines to output.
    fn write_lines<W: Write>(&self, out: &mut W, lines: &[String]);

    // Service methods for ops handlers
    fn kill_project(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn cleanup_project_storage(
        &self,
        context: &crate::context::CliContext,
        command: &Value,
    ) -> Value;
    fn clear_agent_context(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn agent_logs(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn maintenance_status(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn ps_summary(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn export_diagnostic_bundle(
        &self,
        context: &crate::context::CliContext,
        command: &Value,
    ) -> Value;
    fn doctor_storage_summary(&self, context: &crate::context::CliContext) -> Value;
    fn doctor_summary(&self, context: &crate::context::CliContext) -> Value;
    fn list_fault_rules(&self, context: &crate::context::CliContext) -> Value;
    fn arm_fault_rule(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn clear_fault_rule(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn reload_config(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn restart_agent(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn submit_ask(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn ping_target(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn pend_target(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn queue_target(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn trace_target(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn inbox_target(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn ack_reply(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn watch_target(&self, context: &crate::context::CliContext, command: &Value) -> Vec<Value>;
    fn resubmit_message(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn retry_attempt(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn wait_for_replies(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn cancel_job(&self, context: &crate::context::CliContext, command: &Value) -> Value;
    fn validate_config_context(&self, context: &crate::context::CliContext) -> Value;
    fn start_agents(
        &self,
        context: &crate::context::CliContext,
        command: &Value,
        terminal_size: Option<(u16, u16)>,
    ) -> Value;
}

/// Handle the `kill` command.
///
/// Mirrors Python `handle_kill(context, command, out, services)`.
pub fn handle_kill<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.kill_project(context, command);
    services.write_lines(out, &render_kill(&summary));
    0
}

/// Handle the `cleanup` command.
///
/// Mirrors Python `handle_cleanup(context, command, out, services)`.
pub fn handle_cleanup<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.cleanup_project_storage(context, command);
    services.write_lines(out, &render_cleanup(&summary));
    0
}

/// Handle the `clear` command.
///
/// Mirrors Python `handle_clear(context, command, out, services)`.
pub fn handle_clear<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.clear_agent_context(context, command);
    services.write_lines(out, &render_clear(&summary));
    0
}

/// Handle the `logs` command.
///
/// Mirrors Python `handle_logs(context, command, out, services)`.
pub fn handle_logs<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.agent_logs(context, command);
    services.write_lines(out, &render_logs(&summary));
    0
}

/// Handle the `maintenance` command.
///
/// Mirrors Python `handle_maintenance(context, command, out, services)`.
pub fn handle_maintenance<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.maintenance_status(context, command);
    services.write_lines(out, &render_maintenance(&payload));
    let maintenance_status = payload
        .get("maintenance_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if matches!(maintenance_status, "ok" | "degraded") {
        0
    } else {
        2
    }
}

/// Handle the `ps` command.
///
/// Mirrors Python `handle_ps(context, command, out, services)`.
pub fn handle_ps<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.ps_summary(context, command);
    services.write_lines(out, &render_ps(&payload));
    0
}

/// Handle the `doctor` command.
///
/// Mirrors Python `handle_doctor(context, command, out, services)`.
pub fn handle_doctor<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    // Check for bundle mode (command.bundle is true)
    if command
        .get("bundle")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let summary = services.export_diagnostic_bundle(context, command);
        services.write_lines(out, &render_doctor_bundle(&summary));
        return 0;
    }

    // Check for storage mode (command.storage is true)
    if command
        .get("storage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let payload = services.doctor_storage_summary(context);

        // JSON output mode
        if command
            .get("json_output")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let json_str = serde_json::to_string_pretty(&payload).unwrap_or_default();
            writeln!(out, "{}", json_str).ok();
            return 0;
        }

        services.write_lines(out, &render_doctor_storage(&payload));
        return 0;
    }

    // Default doctor mode
    let payload = services.doctor_summary(context);
    services.write_lines(out, &render_doctor(&payload));
    0
}

/// Handle the `fault list` command.
///
/// Mirrors Python `handle_fault_list(context, command, out, services)`.
pub fn handle_fault_list<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
) -> i32 {
    let summary = services.list_fault_rules(context);
    services.write_lines(out, &render_fault_list(&summary));
    0
}

/// Handle the `fault arm` command.
///
/// Mirrors Python `handle_fault_arm(context, command, out, services)`.
pub fn handle_fault_arm<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.arm_fault_rule(context, command);
    services.write_lines(out, &render_fault_arm(&summary));
    0
}

/// Handle the `fault clear` command.
///
/// Mirrors Python `handle_fault_clear(context, command, out, services)`.
pub fn handle_fault_clear<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let summary = services.clear_fault_rule(context, command);
    services.write_lines(out, &render_fault_clear(&summary));
    0
}

/// Handle the `reload` command.
///
/// Mirrors Python `handle_reload(context, command, out, services)`.
pub fn handle_reload<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.reload_config(context, command);
    services.write_lines(out, &render_reload(&payload));
    let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if matches!(status, "ok" | "published" | "noop") {
        0
    } else {
        1
    }
}

/// Handle the `restart` command.
///
/// Mirrors Python `handle_restart(context, command, out, services)`.
pub fn handle_restart<S: Phase2Services, W: Write>(
    services: &S,
    out: &mut W,
    context: &crate::context::CliContext,
    command: &Value,
) -> i32 {
    let payload = services.restart_agent(context, command);
    services.write_lines(out, &render_restart(&payload));
    // Check both restart_status and status fields (Python fallback logic)
    let status = payload
        .get("restart_status")
        .or_else(|| payload.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if status == "ok" {
        0
    } else {
        1
    }
}
