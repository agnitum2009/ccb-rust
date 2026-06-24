//! Mirrors Python `lib/cli/render_runtime/fault_views.py`.

use serde_json::Value;

/// Render a fault list summary.
///
/// Mirrors Python `render_fault_list(summary)`.
pub fn render_fault_list(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        "fault_status: ok".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("rule_count: {}", field(summary, "rule_count")),
    ];
    let rules = summary.get("rules").and_then(|v| v.as_array());
    let empty = rules.map(|a| a.is_empty()).unwrap_or(true);
    if empty {
        lines.push("fault_rule: <none>".to_string());
        return lines;
    }
    if let Some(rules) = rules {
        for rule in rules {
            lines.push(format!(
                "fault_rule: id={} agent={} task={} reason={} remaining={} created={} updated={} error={}",
                field(rule, "rule_id"),
                field(rule, "agent_name"),
                field(rule, "task_id"),
                field(rule, "reason"),
                field(rule, "remaining_count"),
                field(rule, "created_at"),
                field(rule, "updated_at"),
                field(rule, "error_message"),
            ));
        }
    }
    lines
}

/// Render a fault arm summary.
///
/// Mirrors Python `render_fault_arm(summary)`.
pub fn render_fault_arm(summary: &Value) -> Vec<String> {
    vec![
        "fault_status: armed".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("rule_id: {}", field(summary, "rule_id")),
        format!("agent_name: {}", field(summary, "agent_name")),
        format!("task_id: {}", field(summary, "task_id")),
        format!("reason: {}", field(summary, "reason")),
        format!("remaining_count: {}", field(summary, "remaining_count")),
        format!("error_message: {}", field(summary, "error_message")),
    ]
}

/// Render a fault clear summary.
///
/// Mirrors Python `render_fault_clear(summary)`.
pub fn render_fault_clear(summary: &Value) -> Vec<String> {
    let mut lines = vec![
        "fault_status: cleared".to_string(),
        format!("project_id: {}", field(summary, "project_id")),
        format!("target: {}", field(summary, "target")),
        format!("cleared_count: {}", field(summary, "cleared_count")),
    ];
    if let Some(Value::Array(ids)) = summary.get("cleared_rule_ids") {
        for rule_id in ids {
            let text = match rule_id {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            lines.push(format!("cleared_rule_id: {}", text));
        }
    }
    lines
}

fn field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(|v| match v {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}
