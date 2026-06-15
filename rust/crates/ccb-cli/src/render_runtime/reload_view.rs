//! Mirrors Python `lib/cli/render_runtime/reload_view.py`.

use serde_json::Value;

/// Render a reload payload (status + apply/operation/drain/namespace details).
///
/// Mirrors Python `render_reload(payload)`.
pub fn render_reload(payload: &Value) -> Vec<String> {
    let status = field_or(payload, "status", "unknown");
    let mut lines = vec![
        format!("reload_status: {}", status),
        format!("dry_run: {}", bool_field(payload, "dry_run")),
        format!("mutation_enabled: {}", bool_field(payload, "mutation_enabled")),
        format!("plan_class: {}", field(payload, "plan_class")),
        format!("safe_to_apply: {}", bool_field(payload, "safe_to_apply")),
        format!(
            "future_safe_to_apply: {}",
            bool_field(payload, "future_safe_to_apply")
        ),
        format!(
            "old_config_signature: {}",
            field(payload, "old_config_signature")
        ),
        format!(
            "new_config_signature: {}",
            field(payload, "new_config_signature")
        ),
    ];
    lines.extend(reload_apply_lines(payload));
    lines.extend(reload_operation_lines(payload));
    lines.extend(reload_drain_intent_lines(payload));
    if let Some(patch_plan) = payload.get("namespace_patch_plan").filter(|v| v.is_object()) {
        lines.extend(namespace_patch_lines(patch_plan));
    }
    lines.extend(prefixed_values("reload_reason", payload.get("reasons")));
    lines.extend(prefixed_values("reload_warning", payload.get("warnings")));
    lines.extend(prefixed_values("reload_error", payload.get("errors")));
    lines
}

fn reload_apply_lines(payload: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(stage) = payload.get("stage") {
        if !stage.is_null() {
            lines.push(format!("reload_stage: {}", field_value(stage)));
        }
    }
    for (key, label) in [
        ("old_graph_version", "reload_old_graph_version"),
        ("target_graph_version", "reload_target_graph_version"),
        ("published_graph_version", "reload_published_graph_version"),
    ] {
        if let Some(value) = payload.get(key) {
            if !value.is_null() {
                lines.push(format!("{}: {}", label, field_value(value)));
            }
        }
    }
    if let Some(diagnostics) = payload.get("diagnostics").filter(|v| v.is_object()) {
        lines.extend(reload_diagnostic_lines(diagnostics));
    }
    lines
}

fn reload_operation_lines(payload: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(Value::Array(operations)) = payload.get("operations") {
        for operation in operations {
            if operation.is_object() {
                lines.push(format!("reload_operation: {}", operation_line(operation)));
            } else {
                lines.push(format!("reload_operation: {}", field_value(operation)));
            }
        }
    }
    lines
}

fn reload_drain_intent_lines(payload: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(Value::Array(intents)) = payload.get("drain_intents") {
        for intent in intents {
            if intent.is_object() {
                lines.push(format!("reload_drain_intent: {}", drain_intent_line(intent)));
            } else {
                lines.push(format!("reload_drain_intent: {}", field_value(intent)));
            }
        }
    }
    lines
}

fn reload_diagnostic_lines(diagnostics: &Value) -> Vec<String> {
    let mut lines = optional_key_values("reload_diagnostic", diagnostics, &["reason", "message"]);
    for key in [
        "graph_published",
        "lease_or_lifecycle_written",
        "config_watch_started",
        "unload_or_replace_executed",
        "project_view_cache_invalidated",
        "sidebar_refresh_signal_sent",
    ] {
        if let Some(value) = diagnostics.get(key) {
            lines.push(format!(
                "reload_diagnostic: {}={}",
                key,
                value.as_bool().unwrap_or(false)
            ));
        }
    }
    for (key, label) in [
        ("namespace_residue", "reload_namespace_residue"),
        ("runtime_residue", "reload_runtime_residue"),
    ] {
        if let Some(residue) = diagnostics.get(key).filter(|v| v.is_object()) {
            lines.push(format!("{}: {}", label, reload_residue_line(residue)));
        }
    }
    lines
}

fn reload_residue_line(residue: &Value) -> String {
    let mut fields = Vec::new();
    for key in [
        "partial",
        "created_windows",
        "created_panes",
        "agent_panes",
        "sidebar_panes",
        "tool_panes",
        "removed_windows",
        "removed_panes",
        "removed_agents",
        "rollback_actions",
        "requested_agents",
        "mounted_agents",
        "runtime_authority_written_agents",
        "unloaded_agents",
        "runtime_authority_stopped_agents",
        "helper_terminated_agents",
    ] {
        if let Some(value) = residue.get(key) {
            fields.push(format!("{}={}", key, render_value(value)));
        }
    }
    fields.join(" ")
}

fn operation_line(operation: &Value) -> String {
    let mut fields = vec![format!("op={}", field(operation, "op"))];
    fields.extend(present_fields(
        operation,
        &["agent", "window", "from_window", "to_window", "field", "change"],
    ));
    fields.extend(list_fields(operation, &["agents", "fields"]));
    let reason = field(operation, "reason");
    if !reason.is_empty() {
        fields.push(format!("reason={}", reason));
    }
    fields.join(" ")
}

fn drain_intent_line(intent: &Value) -> String {
    let mut fields = vec![format!("intent_kind={}", field(intent, "intent_kind"))];
    fields.extend(present_fields(intent, &["agent", "initial_phase"]));
    if let Some(dry_run_only) = intent.get("dry_run_only") {
        if !dry_run_only.is_null() {
            fields.push(format!("dry_run_only={}", dry_run_only.as_bool().unwrap_or(false)));
        }
    }
    let reason = field(intent, "reason");
    if !reason.is_empty() {
        fields.push(format!("reason={}", reason));
    }
    fields.join(" ")
}

fn namespace_patch_lines(plan: &Value) -> Vec<String> {
    let mut lines = vec![
        format!("reload_namespace_patch_status: {}", field(plan, "status")),
        format!(
            "reload_namespace_patch_apply_deferred: {}",
            bool_field(plan, "apply_deferred")
        ),
    ];
    if let Some(Value::Array(steps)) = plan.get("steps") {
        for step in steps.iter().filter(|s| s.is_object()) {
            lines.push(format!(
                "reload_namespace_patch_step: {}",
                namespace_patch_step_line(step)
            ));
        }
    }
    if let Some(Value::Array(blocked)) = plan.get("blocked_operations") {
        for item in blocked.iter().filter(|s| s.is_object()) {
            lines.push(format!(
                "reload_namespace_patch_blocked: {}",
                namespace_patch_blocked_line(item)
            ));
        }
    }
    lines
}

fn namespace_patch_step_line(step: &Value) -> String {
    let mut fields = vec![format!("action={}", field(step, "action"))];
    fields.extend(present_fields(
        step,
        &["window", "agent", "role", "slot_key", "managed_by", "anchor_agent"],
    ));
    let reason = field(step, "reason");
    if !reason.is_empty() {
        fields.push(format!("reason={}", reason));
    }
    fields.join(" ")
}

fn namespace_patch_blocked_line(blocked: &Value) -> String {
    let mut fields = vec![format!("op={}", field(blocked, "op"))];
    fields.extend(present_fields(blocked, &["agent", "window"]));
    let reason = field(blocked, "reason");
    if !reason.is_empty() {
        fields.push(format!("reason={}", reason));
    }
    fields.join(" ")
}

fn present_fields(record: &Value, keys: &[&str]) -> Vec<String> {
    let mut fields = Vec::new();
    for key in keys {
        if let Some(value) = record.get(key) {
            let present = match value {
                Value::Null => false,
                Value::String(s) => !s.is_empty(),
                _ => true,
            };
            if present {
                fields.push(format!("{}={}", key, field_value(value)));
            }
        }
    }
    fields
}

fn list_fields(record: &Value, keys: &[&str]) -> Vec<String> {
    let mut fields = Vec::new();
    for key in keys {
        if let Some(Value::Array(arr)) = record.get(key) {
            if !arr.is_empty() {
                let joined = arr
                    .iter()
                    .map(field_value)
                    .collect::<Vec<_>>()
                    .join(",");
                fields.push(format!("{}={}", key, joined));
            }
        }
    }
    fields
}

fn optional_key_values(prefix: &str, record: &Value, keys: &[&str]) -> Vec<String> {
    let mut lines = Vec::new();
    for key in keys {
        if let Some(value) = record.get(key) {
            let present = match value {
                Value::Null => false,
                Value::String(s) => !s.is_empty(),
                _ => true,
            };
            if present {
                lines.push(format!("{}: {}={}", prefix, key, field_value(value)));
            }
        }
    }
    lines
}

fn prefixed_values(prefix: &str, values: Option<&Value>) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(Value::Array(arr)) = values {
        for value in arr {
            lines.push(format!("{}: {}", prefix, field_value(value)));
        }
    }
    lines
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Object(obj) => {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            keys.iter()
                .map(|k| format!("{}:{}", k, field_value(&obj[*k])))
                .collect::<Vec<_>>()
                .join(",")
        }
        Value::Array(arr) => arr
            .iter()
            .map(field_value)
            .collect::<Vec<_>>()
            .join(","),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn field_or(value: &Value, key: &str, default: &str) -> String {
    let v = field(value, key);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
}

fn field(value: &Value, key: &str) -> String {
    field_value(value.get(key).unwrap_or(&Value::Null))
}

fn field_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
