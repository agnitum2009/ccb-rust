//! Mirrors Python `lib/cli/render_runtime/common.py`.

use serde_json::Value;
use std::io::Write;

const TERMINAL_OBSERVER_STATUSES: &[&str] = &["completed", "cancelled", "failed", "incomplete"];

/// Strip CCB protocol lines and collapse runs of blank lines.
///
/// Mirrors Python `display_text(value)`. Replaces the two Python regexes
/// (`CCB_(REQ_ID|BEGIN|DONE):` line removal and `\n{3,}` → `\n\n`) with
/// equivalent line-oriented string processing.
pub fn display_text(value: &Value) -> String {
    let raw = match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if raw.is_empty() {
        return String::new();
    }
    let mut kept: Vec<&str> = Vec::new();
    for line in raw.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("CCB_REQ_ID:")
            || trimmed.starts_with("CCB_BEGIN:")
            || trimmed.starts_with("CCB_DONE:")
        {
            continue;
        }
        kept.push(line);
    }
    let joined: String = kept.concat();
    collapse_blank_runs(&joined).trim().to_string()
}

fn collapse_blank_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0usize;
    for ch in text.chars() {
        if ch == '\n' {
            blank_run += 1;
            if blank_run <= 2 {
                out.push(ch);
            }
        } else {
            blank_run = 0;
            out.push(ch);
        }
    }
    out
}

/// Render a mapping as `key: value` lines.
///
/// Mirrors Python `render_mapping(payload)`.
pub fn render_mapping(payload: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(obj) = payload.as_object() {
        for (key, value) in obj {
            lines.push(format!("{}: {}", key, value));
        }
    }
    lines
}

/// Emit observer-surface notice lines.
///
/// Mirrors Python `render_observer_notice(view, terminal, authority)`.
pub fn render_observer_notice(view: &str, terminal: bool, authority: &str) -> Vec<String> {
    let mut lines = vec![
        format!("observer_view: {}", view),
        format!("observer_authority: {}", authority),
        format!("observer_terminal: {}", terminal),
    ];
    if terminal {
        lines.push(
            "observer_notice: weak observer surface; terminal snapshot shown; use ccb trace <id> for authoritative lineage"
                .to_string(),
        );
    } else {
        lines.push(
            "observer_notice: weak observer surface; non-terminal state may change; use ccb trace <id> for lineage when needed"
                .to_string(),
        );
    }
    lines
}

/// Return `true` when `status` names a terminal observer state.
///
/// Mirrors Python `observer_status_is_terminal(status)`.
pub fn observer_status_is_terminal(status: &Value) -> bool {
    let normalized = match status.as_str() {
        Some(s) => s.trim().to_lowercase(),
        None => return false,
    };
    TERMINAL_OBSERVER_STATUSES.contains(&normalized.as_str())
}

/// Render tmux cleanup summary objects as lines.
///
/// Mirrors Python `render_tmux_cleanup_summaries(items)`.
pub fn render_tmux_cleanup_summaries(items: &[Value]) -> Vec<String> {
    let mut lines = Vec::new();
    for item in items {
        let socket_name = cleanup_field(item.get("socket_name"), "<default>");
        let owned = cleanup_csv(item.get("owned_panes"));
        let active = cleanup_csv(item.get("active_panes"));
        let orphaned = cleanup_csv(item.get("orphaned_panes"));
        let killed = cleanup_csv(item.get("killed_panes"));
        lines.push(format!(
            "tmux_cleanup: socket={} owned={} active={} orphaned={} killed={}",
            socket_name, owned, active, orphaned, killed
        ));
    }
    lines
}

/// Render worktree warning objects as lines.
///
/// Mirrors Python `render_worktree_alerts(items)`.
pub fn render_worktree_alerts(items: &[Value]) -> Vec<String> {
    let mut lines = Vec::new();
    for item in items {
        let branch_name = cleanup_field(item.get("branch_name"), "<none>");
        let workspace_path = cleanup_field(item.get("workspace_path"), "<none>");
        lines.push(format!(
            "worktree_warning: agent={} reason={} branch={} dirty={} merged_into_head={} registered={} exists={} path={}",
            str_field(item, "agent_name"),
            str_field(item, "reason"),
            branch_name,
            tri_state(item.get("dirty")),
            tri_state(item.get("merged")),
            tri_state(item.get("registered")),
            tri_state(item.get("exists")),
            workspace_path,
        ));
    }
    lines
}

/// Render worktree retirement objects as lines.
///
/// Mirrors Python `render_worktree_retirements(items)`.
pub fn render_worktree_retirements(items: &[Value]) -> Vec<String> {
    let mut lines = Vec::new();
    for item in items {
        let branch_name = cleanup_field(item.get("branch_name"), "<none>");
        let workspace_path = cleanup_field(item.get("workspace_path"), "<none>");
        lines.push(format!(
            "worktree_retired: agent={} reason={} branch={} removed_agent_state={} path={}",
            str_field(item, "agent_name"),
            str_field(item, "reason"),
            branch_name,
            tri_state(item.get("removed_agent_state")),
            workspace_path,
        ));
    }
    lines
}

/// Join a sequence of values into a CSV string (or `-` when empty).
///
/// Mirrors Python `cleanup_csv(items)`.
pub fn cleanup_csv(items: Option<&Value>) -> String {
    let values: Vec<String> = match items {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| {
                let s = match v {
                    Value::String(s) => s.trim().to_string(),
                    other => other.to_string().trim().to_string(),
                };
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .collect(),
        _ => return "-".to_string(),
    };
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

/// Render an optional scalar field, defaulting when empty.
///
/// Mirrors Python `cleanup_field(value, default)`.
pub fn cleanup_field(value: Option<&Value>, default: &str) -> String {
    let text = match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(other) => other.to_string().trim().to_string(),
        None => return default.to_string(),
    };
    if text.is_empty() {
        default.to_string()
    } else {
        text
    }
}

fn tri_state(value: Option<&Value>) -> &'static str {
    match value {
        Some(Value::Bool(true)) => "true",
        Some(Value::Bool(false)) => "false",
        _ => "unknown",
    }
}

fn str_field(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Write each line to `out`, mirroring Python `write_lines(out, lines)`.
pub fn write_lines<W: Write>(out: &mut W, lines: &[String]) {
    for line in lines {
        let _ = writeln!(out, "{}", line);
    }
}
