use crate::types::{SessionStats, ToolExecution, TransferContext};
use chrono::Local;

const PROVIDER_LABELS: &[(&str, &str)] = &[
    ("auto", "Auto"),
    ("claude", "Claude"),
    ("codex", "Codex"),
    ("droid", "Droid"),
    ("gemini", "Gemini"),
    ("opencode", "OpenCode"),
];

/// Format context for output to other agents.
#[derive(Debug, Clone)]
pub struct ContextFormatter {
    pub max_tokens: u32,
}

impl Default for ContextFormatter {
    fn default() -> Self {
        Self::new(8000)
    }
}

impl ContextFormatter {
    pub const CHARS_PER_TOKEN: u32 = 4;

    pub fn new(max_tokens: u32) -> Self {
        Self { max_tokens }
    }

    /// Convert provider key to display label.
    fn effective_provider(context: &TransferContext) -> &str {
        if context.source_provider.is_empty() {
            "claude"
        } else {
            &context.source_provider
        }
    }

    pub fn provider_label(provider: Option<&str>) -> String {
        let key = provider.unwrap_or("claude").trim().to_lowercase();
        PROVIDER_LABELS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, label)| (*label).to_string())
            .unwrap_or_else(|| {
                provider
                    .unwrap_or("claude")
                    .trim()
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_uppercase().collect::<String>()
                        } else {
                            c.to_string()
                        }
                    })
                    .collect()
            })
    }

    /// Estimate token count from text length.
    pub fn estimate_tokens(&self, text: &str) -> u32 {
        (text.len() as u32) / Self::CHARS_PER_TOKEN
    }

    /// Keep the most recent conversation pairs that fit within `max_tokens`.
    pub fn truncate_to_limit(
        &self,
        conversations: &[(String, String)],
        max_tokens: Option<u32>,
    ) -> Vec<(String, String)> {
        let limit = max_tokens.unwrap_or(self.max_tokens);
        let mut result = Vec::new();
        let mut total_tokens = 0u32;

        for (user_msg, assistant_msg) in conversations.iter().rev() {
            let pair_tokens = self.estimate_tokens(&(user_msg.clone() + assistant_msg));
            if total_tokens + pair_tokens > limit {
                break;
            }
            result.push((user_msg.clone(), assistant_msg.clone()));
            total_tokens += pair_tokens;
        }

        result.reverse();
        result
    }

    /// Format context as Markdown.
    pub fn format_markdown(&self, context: &TransferContext, detailed: bool) -> String {
        let provider = Self::provider_label(Some(Self::effective_provider(context)));
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut lines: Vec<String> = vec![
            format!("## Context Transfer from {provider} Session"),
            String::new(),
            format!("**IMPORTANT**: This is a context handoff from a {provider} session."),
            "The previous AI assistant completed the work described below.".to_string(),
            "Please review and continue from where it left off.".to_string(),
            String::new(),
            format!("**Source Provider**: {provider}"),
            format!("**Source Session**: {}", context.source_session_id),
            format!("**Transferred**: {timestamp}"),
            format!("**Conversations**: {}", context.conversations.len()),
            String::new(),
            "---".to_string(),
            String::new(),
        ];

        if let Some(stats) = &context.stats {
            lines.extend(format_stats_section(stats, detailed));
        }

        lines.extend([
            "### Previous Conversation Context".to_string(),
            String::new(),
        ]);

        for (index, (user_msg, assistant_msg)) in context.conversations.iter().enumerate() {
            lines.extend([
                format!("#### Turn {}", index + 1),
                format!("**User**: {user_msg}"),
                String::new(),
                format!("**Assistant**: {assistant_msg}"),
                String::new(),
                "---".to_string(),
                String::new(),
            ]);
        }

        lines.push(
            "**Action Required**: Review the above context and continue the work.".to_string(),
        );
        lines.join("\n")
    }

    /// Format context as plain text.
    pub fn format_plain(&self, context: &TransferContext) -> String {
        let provider = Self::provider_label(Some(Self::effective_provider(context)));
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut lines: Vec<String> = vec![
            format!("=== Context Transfer from {provider} ==="),
            format!("Provider: {provider}"),
            format!("Session: {}", context.source_session_id),
            format!("Transferred: {timestamp}"),
            format!("Conversations: {}", context.conversations.len()),
            String::new(),
            "=== Previous Conversation ===".to_string(),
            String::new(),
        ];

        for (index, (user_msg, assistant_msg)) in context.conversations.iter().enumerate() {
            lines.extend([
                format!("--- Turn {} ---", index + 1),
                format!("User: {user_msg}"),
                String::new(),
                format!("Assistant: {assistant_msg}"),
                String::new(),
            ]);
        }

        lines.push("=== End of Context ===".to_string());
        lines.join("\n")
    }

    /// Format context as JSON.
    pub fn format_json(&self, context: &TransferContext) -> String {
        let provider = Self::effective_provider(context).trim().to_lowercase();
        let data = serde_json::json!({
            "source_provider": provider,
            "source_session_id": context.source_session_id,
            "transferred_at": Local::now().to_rfc3339(),
            "token_estimate": context.token_estimate,
            "conversations": context.conversations.iter().map(|(u, a)| serde_json::json!({"user": u, "assistant": a})).collect::<Vec<_>>(),
            "metadata": context.metadata,
        });
        serde_json::to_string_pretty(&data).unwrap_or_default()
    }

    /// Format context with the requested format.
    pub fn format(&self, context: &TransferContext, fmt: &str, detailed: bool) -> String {
        match fmt {
            "plain" => self.format_plain(context),
            "json" => self.format_json(context),
            _ => self.format_markdown(context, detailed),
        }
    }
}

// ---------------------------------------------------------------------------
// Stats formatting
// ---------------------------------------------------------------------------

pub fn format_stats_section(stats: &SessionStats, detailed: bool) -> Vec<String> {
    if stats.tool_calls.is_empty()
        && stats.files_written.is_empty()
        && stats.files_edited.is_empty()
        && stats.files_read.is_empty()
        && stats.tasks_created == 0
        && stats.tool_executions.is_empty()
    {
        return Vec::new();
    }

    let mut lines = vec!["### Session Activity Summary".to_string(), String::new()];
    lines.extend(tool_calls_section(stats));
    lines.extend(path_section(
        "**Files Created/Written:**",
        &stats.files_written,
        if detailed { 50 } else { 15 },
        true,
    ));
    lines.extend(path_section(
        "**Files Edited:**",
        &stats.files_edited,
        if detailed { 30 } else { 10 },
        false,
    ));
    lines.extend(path_section(
        "**Files Read:**",
        &stats.files_read,
        if detailed { 30 } else { 10 },
        true,
    ));
    if stats.tasks_created > 0 {
        lines.push(format!(
            "**Tasks:** {}/{} completed",
            stats.tasks_completed, stats.tasks_created
        ));
        lines.push(String::new());
    }
    if !stats.tool_executions.is_empty() {
        lines.extend(format_tool_executions(&stats.tool_executions, detailed));
    }
    lines.extend(["---".to_string(), String::new()]);
    lines
}

fn tool_calls_section(stats: &SessionStats) -> Vec<String> {
    if stats.tool_calls.is_empty() {
        return Vec::new();
    }
    let mut entries: Vec<(&String, &u32)> = stats.tool_calls.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    let mut lines = vec!["**Tool Calls:**".to_string()];
    for (name, count) in entries {
        lines.push(format!("- {name}: {count}"));
    }
    lines.push(String::new());
    lines
}

fn path_section(title: &str, paths: &[String], limit: usize, truncate_notice: bool) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![title.to_string()];
    for path in paths.iter().take(limit) {
        lines.push(format!("- `{path}`"));
    }
    if truncate_notice && paths.len() > limit {
        lines.push(format!("- ... and {} more", paths.len() - limit));
    }
    lines.push(String::new());
    lines
}

// ---------------------------------------------------------------------------
// Tool execution formatting
// ---------------------------------------------------------------------------

pub fn format_tool_executions(executions: &[ToolExecution], detailed: bool) -> Vec<String> {
    if detailed {
        detailed_tool_executions(executions)
    } else {
        recent_tool_executions(executions)
    }
}

fn detailed_tool_executions(executions: &[ToolExecution]) -> Vec<String> {
    let mut lines = vec!["**All Tool Executions:**".to_string(), String::new()];
    for (index, execution) in executions.iter().enumerate() {
        let error_marker = if execution.is_error { " ❌" } else { "" };
        lines.push(format!(
            "#### {}. {}{}",
            index + 1,
            execution.name,
            error_marker
        ));
        let input_text = format_tool_input(&execution.name, &execution.input);
        if !input_text.is_empty() {
            lines.push(format!("- **Input**: `{input_text}`"));
        }
        if let Some(result) = &execution.result {
            lines.extend([
                "- **Result**:".to_string(),
                "```".to_string(),
                result.clone(),
                "```".to_string(),
            ]);
        }
        lines.push(String::new());
    }
    lines
}

fn recent_tool_executions(executions: &[ToolExecution]) -> Vec<String> {
    let mut lines = vec!["**Recent Tool Executions:**".to_string(), String::new()];
    let mut shown = 0usize;
    for execution in executions.iter().rev() {
        if ["Read", "Glob", "Grep"].contains(&execution.name.as_str()) {
            continue;
        }
        let error_marker = if execution.is_error { " ❌" } else { "" };
        lines.push(format!("- **{}{}**", execution.name, error_marker));
        let input_text = format_tool_input(&execution.name, &execution.input);
        if !input_text.is_empty() {
            lines.push(format!("  - Input: {input_text}"));
        }
        if let Some(result) = &execution.result {
            lines.push(format!("  - Result: `{}`", result_preview(result)));
        }
        shown += 1;
        if shown >= 5 {
            break;
        }
    }
    if executions.len() > 5 {
        lines.push(format!("- ... and {} more", executions.len() - 5));
    }
    lines.push(String::new());
    lines
}

fn result_preview(result: &str) -> String {
    let preview: String = result
        .chars()
        .take(150)
        .collect::<String>()
        .replace('\n', " ");
    if result.len() > 150 {
        format!("{preview}...")
    } else {
        preview
    }
}

pub fn format_tool_input(name: &str, input: &serde_json::Value) -> String {
    if input.is_null() || (input.is_object() && input.as_object().unwrap().is_empty()) {
        return String::new();
    }
    match name {
        "Write" | "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Bash" => {
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if cmd.len() > 80 {
                format!("{}...", &cmd[..80])
            } else {
                cmd
            }
        }
        "TaskCreate" => input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "TaskUpdate" => format!(
            "#{} -> {}",
            input.get("taskId").and_then(|v| v.as_str()).unwrap_or(""),
            input.get("status").and_then(|v| v.as_str()).unwrap_or("")
        ),
        _ => String::new(),
    }
}
