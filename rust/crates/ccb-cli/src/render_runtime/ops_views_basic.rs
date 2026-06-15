//! Mirrors Python `lib/cli/render_runtime/ops_views_basic.py`.

use serde_json::Value;
use std::collections::HashMap;

use super::common::{cleanup_csv, cleanup_field, render_tmux_cleanup_summaries};
use super::ops_views_common::binding_line;

/// Render a config validation summary.
///
/// Mirrors Python `render_config_validate(summary)`.
pub fn render_config_validate(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        "config_status: valid".to_string(),
        format!("project: {}", field(summary, "project_root")),
        format!("project_id: {}", field(summary, "project_id")),
        format!("config_source_kind: {}", field(summary, "source_kind")),
        format!(
            "config_source: {}",
            field_or(summary, "source", "<builtin>")
        ),
        format!(
            "used_builtin_default: {}",
            bool_field(summary, "used_builtin_default")
        ),
        format!(
            "default_agents: {}",
            csv_field(summary, "default_agents")
        ),
        format!("agents: {}", csv_field(summary, "agent_names")),
        format!(
            "cmd_enabled: {}",
            bool_field(summary, "cmd_enabled")
        ),
        format!("layout: {}", field(summary, "layout_spec")),
    ];

    if let Some(Value::Array(warnings)) = summary.get("style_warnings") {
        for warning in warnings {
            if let Some(w) = warning.as_str() {
                lines.push(format!("config_warning: {}", w));
            }
        }
    }

    lines
}

/// Render a start summary.
///
/// Mirrors Python `render_start(summary)`.
pub fn render_start(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        "start_status: ok".to_string(),
        format!("project: {}", field(summary, "project_root")),
        format!("project_id: {}", field(summary, "project_id")),
        format!(
            "ccbd_started: {}",
            bool_field(summary, "daemon_started")
        ),
        format!("socket_path: {}", field(summary, "socket_path")),
        format!("agents: {}", csv_field(summary, "started")),
    ];

    if let Some(heartbeat) = summary.get("maintenance_heartbeat").and_then(|v| v.as_object()) {
        let mut details = vec![
            format!("status={}", field(heartbeat, "maintenance_status")),
            format!("action={}", field(heartbeat, "action")),
        ];

        if heartbeat.get("runner_status").is_some() {
            details.push(format!("runner_status={}", field(heartbeat, "runner_status")));
        }
        if heartbeat.get("tick_status").is_some() {
            details.push(format!("tick_status={}", field(heartbeat, "tick_status")));
        }

        lines.push(format!("maintenance_heartbeat: {}", details.join(" ")));

        let reason = field(heartbeat, "reason");
        if !reason.is_empty() {
            lines.push(format!("maintenance_heartbeat_reason: {}", reason));
        }
    }

    if let Some(Value::Array(cleanup_summaries)) = summary.get("cleanup_summaries") {
        lines.extend(render_tmux_cleanup_summaries(cleanup_summaries));
    }

    lines
}

/// Render a logs summary.
///
/// Mirrors Python `render_logs(summary)`.
pub fn render_logs(summary: &Value) -> Vec<String> {
    let entries = summary.get("entries").and_then(|v| v.as_array());
    let entries: Vec<&Value> = entries.map(|a| a.iter().collect()).unwrap_or_default();

    let mut lines = vec![
        "logs_status: ok".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("agent_name: {}", field(summary, "agent_name")),
        format!("provider: {}", field(summary, "provider")),
        format!("runtime_ref: {}", field(summary, "runtime_ref")),
        format!("session_ref: {}", field(summary, "session_ref")),
        format!("log_count: {}", entries.len()),
    ];

    if entries.is_empty() {
        lines.push("log: <none>".to_string());
        return lines;
    }

    for entry in entries {
        lines.push(format!(
            "log: {} {}",
            field(entry, "source"),
            field(entry, "path")
        ));
        if let Some(Value::Array(lines_data)) = entry.get("lines") {
            for line in lines_data {
                if let Some(l) = line.as_str() {
                    lines.push(format!("log_line: {}", l));
                }
            }
        }
    }

    lines
}

/// Render a doctor bundle summary.
///
/// Mirrors Python `render_doctor_bundle(summary)`.
pub fn render_doctor_bundle(summary: &Value) -> Vec<String> {
    vec![
        "doctor_bundle_status: ok".to_string(),
        format!("project: {}", field(summary, "project_root")),
        format!("project_id: {}", field(summary, "project_id")),
        format!("bundle_id: {}", field(summary, "bundle_id")),
        format!("bundle_path: {}", field(summary, "bundle_path")),
        format!("file_count: {}", field(summary, "file_count")),
        format!(
            "included_count: {}",
            field(summary, "included_count")
        ),
        format!(
            "missing_count: {}",
            field(summary, "missing_count")
        ),
        format!(
            "truncated_count: {}",
            field(summary, "truncated_count")
        ),
        format!("doctor_error: {}", field(summary, "doctor_error")),
    ]
}

/// Render a cleanup summary.
///
/// Mirrors Python `render_cleanup(summary)`.
pub fn render_cleanup(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        format!("cleanup_status: {}", field(summary, "status")),
        format!("project_root: {}", field(summary, "project_root")),
        format!("project_id: {}", field(summary, "project_id")),
        format!(
            "cleanup_deleted_bytes: {}",
            field(summary, "deleted_bytes")
        ),
        format!(
            "cleanup_deleted_count: {}",
            field(summary, "deleted_count")
        ),
        format!(
            "cleanup_skipped_count: {}",
            field(summary, "skipped_count")
        ),
    ];

    if let Some(Value::Array(actions)) = summary.get("actions") {
        for action in actions {
            lines.push(format!(
                "cleanup_action: provider={} kind={} bytes={} reason={} path={}",
                field(action, "provider"),
                field(action, "kind"),
                field(action, "bytes_removed"),
                field(action, "reason"),
                field(action, "path")
            ));
        }
    }

    if let Some(Value::Array(skipped)) = summary.get("skipped") {
        for item in skipped {
            lines.push(format!(
                "cleanup_skipped: provider={} reason={} path={}",
                field(item, "provider"),
                field(item, "reason"),
                field(item, "path")
            ));
        }
    }

    lines
}

/// Render a clear summary.
///
/// Mirrors Python `render_clear(summary)`.
pub fn render_clear(summary: &Value) -> Vec<String> {
    let results = summary
        .get("results")
        .and_then(|v| v.as_array())
        .unwrap_or(&[]);

    let mut cleared_count = 0;
    let mut skipped_count = 0;
    let mut failed_count = 0;

    for item in results {
        let status = field(item, "status");
        match status.as_str() {
            "cleared" => cleared_count += 1,
            "skipped" => skipped_count += 1,
            "failed" => failed_count += 1,
            _ => {}
        }
    }

    let mut lines = vec![
        format!("clear_status: {}", field_or(summary, "status", "unknown")),
        format!("cleared_count: {}", cleared_count),
        format!("skipped_count: {}", skipped_count),
        format!("failed_count: {}", failed_count),
    ];

    for item in results {
        let agent = field(item, "agent");
        let status = field(item, "status");
        let pane_id = field(item, "pane_id");
        let reason = field(item, "reason");

        let mut detail = format!("agent={} status={}", agent, status);
        if !pane_id.is_empty() {
            detail.push_str(&format!(" pane_id={}", pane_id));
        }
        if !reason.is_empty() {
            detail.push_str(&format!(" reason={}", reason));
        }
        lines.push(format!("clear_agent: {}", detail));
    }

    lines
}

/// Render a restart summary.
///
/// Mirrors Python `render_restart(summary)`.
pub fn render_restart(summary: &Value) -> Vec<String> {
    let status = field_or(summary, "restart_status", &field_or(summary, "status", "unknown"));

    let mut lines = vec![
        format!("restart_status: {}", status),
        format!("agent_name: {}", field(summary, "agent_name")),
    ];

    let restartable_agents = summary
        .get("restartable_agents")
        .and_then(|v| v.as_array())
        .unwrap_or(&[]);

    let agents: Vec<String> = restartable_agents
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect();

    if !agents.is_empty() {
        lines.push(format!("restartable_agents: {}", agents.join(", ")));
    }

    let reason = field(summary, "reason");
    if !reason.is_empty() {
        lines.push(format!("reason: {}", reason));
    }

    if let Some(busy_gate) = summary.get("busy_gate").and_then(|v| v.as_object()) {
        lines.push(restart_busy_gate_line(busy_gate));
    }

    if let Some(Value::Array(blockers)) = summary.get("blockers") {
        for blocker in blockers {
            if let Some(obj) = blocker.as_object() {
                let reason_text = field(blocker, "reason");
                let detail = field(blocker, "detail");
                let mut line = format!("blocker: reason={}", reason_text);
                if !detail.is_empty() {
                    line.push_str(&format!(" detail={}", detail));
                }
                lines.push(line);
            } else {
                lines.push(format!("blocker: {}", field_value(blocker)));
            }
        }
    }

    if let Some(old_runtime) = summary.get("old_runtime").and_then(|v| v.as_object()) {
        lines.push(format!("old_runtime: {}", runtime_evidence_text(old_runtime)));
    }

    if let Some(new_runtime) = summary.get("new_runtime").and_then(|v| v.as_object()) {
        lines.push(format!("new_runtime: {}", runtime_evidence_text(new_runtime)));
    }

    if let Some(result) = summary.get("result").and_then(|v| v.as_object()) {
        lines.push(format!("restart_result: {}", flat_mapping_text(result)));
    }

    let error = field(summary, "error");
    if !error.is_empty() {
        lines.push(format!("error: {}", error));
    }

    lines
}

/// Render a maintenance payload.
///
/// Mirrors Python `render_maintenance(payload)`.
pub fn render_maintenance(payload: &Value) -> Vec<String> {
    let status = field_or(payload, "maintenance_status", "unknown");
    let mut lines = vec![format!("maintenance_status: {}", status)];

    let action = field(payload, "action");
    if !action.is_empty() {
        lines.push(format!("action: {}", action));
    }

    let reason = field(payload, "reason");
    if !reason.is_empty() {
        lines.push(format!("reason: {}", reason));
    }

    if status == "not_implemented" {
        return lines;
    }

    let runner_status = field(payload, "runner_status");
    if !runner_status.is_empty() {
        lines.extend(vec![
            format!("runner_status: {}", runner_status),
            format!(
                "runner_started: {}",
                render_optional(payload.get("runner_started"))
            ),
            format!(
                "runner_id: {}",
                render_optional(payload.get("runner_id"))
            ),
            format!(
                "runner_pid: {}",
                render_optional(payload.get("runner_pid"))
            ),
            format!(
                "runner_exit_reason: {}",
                render_optional(payload.get("runner_exit_reason"))
            ),
            format!(
                "runner_iterations: {}",
                render_optional(payload.get("runner_iterations"))
            ),
        ]);
    }

    let tick_status = field(payload, "tick_status");
    if !tick_status.is_empty() {
        lines.extend(vec![
            format!("tick_status: {}", tick_status),
            format!(
                "tick_source_kind: {}",
                field(payload, "tick_source_kind")
            ),
            format!(
                "tick_recommended_action: {}",
                field(payload, "tick_recommended_action")
            ),
            format!(
                "tick_needs_user: {}",
                render_value(payload.get("tick_needs_user"))
            ),
            format!(
                "tick_next_heartbeat_after_s: {}",
                render_optional(payload.get("tick_next_heartbeat_after_s"))
            ),
            format!(
                "status_written: {}",
                render_value(payload.get("status_written"))
            ),
            format!(
                "schedule_written: {}",
                render_value(payload.get("schedule_written"))
            ),
            format!(
                "activation_written: {}",
                render_value(payload.get("activation_written"))
            ),
            format!(
                "tick_activation_status: {}",
                render_optional(payload.get("tick_activation_status"))
            ),
            format!(
                "tick_activation_id: {}",
                render_optional(payload.get("tick_activation_id"))
            ),
            format!(
                "tick_activation_job_id: {}",
                render_optional(payload.get("tick_activation_job_id"))
            ),
        ]);

        if let Some(summary) = payload.get("tick_summary").and_then(|v| v.as_object()) {
            lines.extend(maintenance_summary_lines("tick_summary", summary));
        }

        if let Some(Value::Array(evidence)) = payload.get("tick_evidence") {
            lines.push(format!("tick_evidence_count: {}", evidence.len()));
            for item in evidence.iter().take(5) {
                if let Some(obj) = item.as_object() {
                    lines.push(maintenance_evidence_line("tick_evidence", obj));
                }
            }
        }
    }

    lines.extend(vec![
        format!("project: {}", field(payload, "project")),
        format!("project_id: {}", field(payload, "project_id")),
        format!(
            "config_source_kind: {}",
            field(payload, "config_source_kind")
        ),
        format!(
            "config_source: {}",
            field_or(payload, "config_source", "<builtin>")
        ),
        format!(
            "heartbeat_enabled: {}",
            render_value(payload.get("enabled"))
        ),
        format!("heartbeat_assessor: {}", field(payload, "assessor")),
        format!(
            "heartbeat_assessor_present: {}",
            render_value(payload.get("assessor_present"))
        ),
        format!(
            "heartbeat_interval_s: {}",
            field(payload, "interval_s")
        ),
        format!(
            "heartbeat_min_interval_s: {}",
            field(payload, "min_interval_s")
        ),
        format!(
            "heartbeat_unknown_streak_cap: {}",
            field(payload, "unknown_streak_cap")
        ),
        format!(
            "heartbeat_escalation_policy: {}",
            field(payload, "escalation_policy")
        ),
        format!(
            "heartbeat_startup_ensure: {}",
            render_value(payload.get("startup_ensure"))
        ),
    ]);

    if let Some(schedule) = payload.get("schedule").and_then(|v| v.as_object()) {
        lines.extend(maintenance_record_lines("schedule", schedule));
    }

    if let Some(last_status) = payload.get("last_status").and_then(|v| v.as_object()) {
        lines.extend(maintenance_record_lines("last_status", last_status));
    }

    if let Some(runner) = payload.get("runner").and_then(|v| v.as_object()) {
        lines.extend(maintenance_record_lines("runner", runner));
    }

    if let Some(last_activation) = payload.get("last_activation").and_then(|v| v.as_object()) {
        lines.extend(maintenance_record_lines("last_activation", last_activation));
    }

    lines
}

/// Render a kill summary.
///
/// Mirrors Python `render_kill(summary)`.
pub fn render_kill(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        "kill_status: ok".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("state: {}", field(summary, "state")),
        format!("socket_path: {}", field(summary, "socket_path")),
        format!("forced: {}", bool_field(summary, "forced")),
    ];

    if let Some(Value::Array(cleanup_summaries)) = summary.get("cleanup_summaries") {
        lines.extend(render_tmux_cleanup_summaries(cleanup_summaries));
    }

    lines
}

/// Render a ps payload.
///
/// Mirrors Python `render_ps(payload)`.
pub fn render_ps(payload: &Value) -> Vec<String> {
    let mut lines = vec![
        format!("project_id: {}", field(payload, "project_id")),
        format!("ccbd_state: {}", field(payload, "ccbd_state")),
    ];

    if let Some(Value::Array(agents)) = payload.get("agents") {
        for agent in agents {
            lines.push(format!(
                "agent: name={} state={} provider={} queue={}",
                field(agent, "agent_name"),
                field(agent, "state"),
                field(agent, "provider"),
                field(agent, "queue_depth")
            ));
            lines.push(binding_line(agent));
        }
    }

    lines
}

/// Generate maintenance record lines with a prefix.
///
/// Mirrors Python `_maintenance_record_lines(prefix, payload)`.
fn maintenance_record_lines(prefix: &str, payload: &Value) -> Vec<String> {
    let mut lines = vec![
        format!("{}_state: {}", prefix, field(payload, "state")),
        format!("{}_path: {}", prefix, field(payload, "path")),
    ];

    let error = field(payload, "error");
    if !error.is_empty() {
        lines.push(format!("{}_error: {}", prefix, error));
    }

    if let Some(record) = payload.get("record").and_then(|v| v.as_object()) {
        for key in [
            "next_run_at",
            "reason",
            "updated_at",
            "updated_by",
            "last_tick_status",
            "last_tick_at",
            "last_ok_at",
            "last_error",
            "unknown_streak",
            "source_kind",
            "recommended_action",
            "next_heartbeat_after_s",
            "needs_user",
            "last_activation_status",
            "last_activation_id",
            "last_activation_job_id",
            "last_activation_target",
            "last_activation_dedup_key",
            "runner_id",
            "pid",
            "state",
            "started_at",
            "last_seen_at",
            "last_wake_at",
            "last_tick_at",
            "last_tick_status",
            "observed_next_run_at",
            "sleep_until",
            "exit_reason",
            "activation_id",
            "status",
            "condition_kind",
            "trigger_kind",
            "source",
            "observed_at",
            "target_agent",
            "delivery_mode",
            "payload_kind",
            "dedup_key",
            "job_id",
            "submitted_at",
            "suppressed_reason",
            "repeat_count",
        ] {
            if record.contains_key(key) {
                lines.push(format!(
                    "_{}_: {}",
                    prefix,
                    key,
                    render_value(record.get(key))
                ));
            }
        }

        if let Some(summary) = record.get("summary").and_then(|v| v.as_object()) {
            lines.extend(maintenance_summary_lines(&format!("{}_summary", prefix), summary));
        }

        if let Some(Value::Array(evidence)) = record.get("evidence") {
            lines.push(format!("{}_evidence_count: {}", prefix, evidence.len()));
        }
    }

    lines
}

/// Generate maintenance summary lines with a prefix.
///
/// Mirrors Python `_maintenance_summary_lines(prefix, payload)`.
fn maintenance_summary_lines(prefix: &str, payload: &Value) -> Vec<String> {
    let mut lines = Vec::new();

    for key in [
        "source_kind",
        "ccbd_state",
        "agent_count",
        "active_agent_count",
        "pending_agent_count",
        "idle_agent_count",
        "offline_agent_count",
        "failed_agent_count",
        "concern_agent_count",
        "unknown_agent_count",
        "comms_count",
        "active_comms_count",
        "concern_comms_count",
        "failing_comms_count",
        "suspicion_count",
        "fallback_error",
    ] {
        if payload.get(key).is_some() {
            lines.push(format!(
                "_{}_: {}",
                prefix,
                key,
                render_value(payload.get(key))
            ));
        }
    }

    lines
}

/// Generate a maintenance evidence line.
///
/// Mirrors Python `_maintenance_evidence_line(prefix, payload)`.
fn maintenance_evidence_line(prefix: &str, payload: &Value) -> String {
    let mut parts = vec![format!("{}:", prefix)];

    for key in [
        "health",
        "kind",
        "condition_kind",
        "agent",
        "job_id",
        "target",
        "reason",
        "source",
        "status",
        "ccbd_state",
        "confidence",
    ] {
        if let Some(value) = payload.get(key) {
            if !value.is_null() {
                let rendered = render_value(Some(value));
                if !rendered.is_empty() && rendered != "None" {
                    parts.push(format!("{}={}", key, rendered));
                }
            }
        }
    }

    parts.join(" ")
}

/// Render an optional value, or "<none>" if None/null.
///
/// Mirrors Python `_render_optional(value)`.
fn render_optional(value: Option<&Value>) -> String {
    match value {
        Some(Value::Null) | None => "<none>".to_string(),
        Some(v) => render_value(Some(v)),
    }
}

/// Generate a restart busy gate line.
///
/// Mirrors Python `_restart_busy_gate_line(gate)`.
fn restart_busy_gate_line(gate: &Value) -> String {
    let mut fields = HashMap::new();
    fields.insert("passed", render_value(gate.get("passed")));
    fields.insert("runtime_state", render_value(gate.get("runtime_state")));
    fields.insert(
        "runtime_queue_depth",
        render_value(gate.get("runtime_queue_depth")),
    );
    fields.insert("queue_depth", render_value(gate.get("queue_depth")));
    fields.insert(
        "pending_reply_count",
        render_value(gate.get("pending_reply_count")),
    );
    fields.insert("active_job_id", render_value(gate.get("active_job_id")));
    fields.insert(
        "active_inbound_event_id",
        render_value(gate.get("active_inbound_event_id")),
    );
    fields.insert(
        "pending_callback_count",
        render_value(gate.get("pending_callback_count")),
    );

    format!("restart_busy_gate: {}", flat_mapping_text(&fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))
}

/// Generate runtime evidence text.
///
/// Mirrors Python `_runtime_evidence_text(evidence)`.
fn runtime_evidence_text(evidence: &Value) -> String {
    let mut fields = HashMap::new();
    fields.insert("state", render_value(evidence.get("state")));
    fields.insert("health", render_value(evidence.get("health")));
    fields.insert("pane_id", render_value(evidence.get("pane_id")));
    fields.insert(
        "active_pane_id",
        render_value(evidence.get("active_pane_id")),
    );
    fields.insert(
        "runtime_ref",
        render_value(evidence.get("runtime_ref")),
    );
    fields.insert(
        "session_ref",
        render_value(evidence.get("session_ref")),
    );
    fields.insert(
        "runtime_pid",
        render_value(evidence.get("runtime_pid")),
    );
    fields.insert(
        "restart_count",
        render_value(evidence.get("restart_count")),
    );

    flat_mapping_text(&fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// Flatten a mapping to "key=value key=value" text.
///
/// Mirrors Python `_flat_mapping_text(payload)`.
fn flat_mapping_text(payload: &HashMap<String, String>) -> String {
    payload
        .iter()
        .map(|(key, value)| format!("{}={}", key, value))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render a value as a string.
///
/// Mirrors Python `_render_value(value)`.
fn render_value(value: Option<&Value>) -> String {
    match value {
        None => "None".to_string(),
        Some(Value::Null) => "None".to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|v| v.to_string().replace('\n', "\\n"))
            .collect::<Vec<_>>()
            .join(","),
        Some(v) => v.to_string().replace('\n', "\\n"),
    }
}

/// Get a string field from a value.
fn field(value: &Value, key: &str) -> String {
    field_value(value.get(key).unwrap_or(&Value::Null))
}

/// Get a string field or a default if empty.
fn field_or(value: &Value, key: &str, default: &str) -> String {
    let v = field(value, key);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
}

/// Get a boolean field from a value.
fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Get a CSV-joined field from an array.
fn csv_field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// Convert a Value to a string representation.
fn field_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
