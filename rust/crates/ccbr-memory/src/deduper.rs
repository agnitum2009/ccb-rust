use crate::types::ConversationEntry;
use regex::Regex;

/// Protocol markers to remove.
pub const PROTOCOL_PATTERNS: &[&str] = &[
    r"^\s*CCB_REQ_ID:\s*\d{8}-\d{6}-\d{3}-\d+-\d+\s*$",
    r"^\s*CCB_BEGIN:\s*\d{8}-\d{6}-\d{3}-\d+-\d+\s*$",
    r"^\s*CCB_DONE:\s*\d{8}-\d{6}-\d{3}-\d+-\d+\s*$",
    r"^\s*\[CCB_ASYNC_SUBMITTED[^\]]*\].*$",
    r"^\s*CCB_CALLER=\w+\s*$",
    r"^\s*\[Request interrupted by user for tool use\]\s*$",
    r"^\s*The user doesn't want to proceed with this tool use\..*$",
    r"^\s*User rejected tool use\s*$",
];

/// System noise patterns to remove (multiline).
pub const SYSTEM_NOISE_PATTERNS: &[&str] = &[
    r"<system-reminder>.*?</system-reminder>",
    r"<env>.*?</env>",
    r"<rules>.*?</rules>",
    r"<!-- CCB_CONFIG_START -->.*?<!-- CCB_CONFIG_END -->",
    r"<local-command-caveat>.*?</local-command-caveat>",
    r"\[CCB_ASYNC_SUBMITTED[^\]]*\][\s\S]*?(?:\n\n|\z)",
];

/// Clean and deduplicate conversation content.
#[derive(Debug, Clone)]
pub struct ConversationDeduper {
    protocol_re: Vec<Regex>,
    noise_re: Vec<Regex>,
}

impl Default for ConversationDeduper {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationDeduper {
    pub fn new() -> Self {
        Self {
            protocol_re: PROTOCOL_PATTERNS
                .iter()
                .map(|p| Regex::new(p).expect("builtin protocol pattern"))
                .collect(),
            noise_re: SYSTEM_NOISE_PATTERNS
                .iter()
                .map(|p| Regex::new(&format!("(?s){}", p)).expect("builtin noise pattern"))
                .collect(),
        }
    }

    /// Remove CCB protocol markers from text.
    pub fn strip_protocol_markers(&self, text: &str) -> String {
        text.lines()
            .filter(|line| !self.matches_protocol_marker(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Remove system noise tags from text.
    pub fn strip_system_noise(&self, text: &str) -> String {
        let mut result = text.to_string();
        for pattern in &self.noise_re {
            result = pattern.replace_all(&result, "").to_string();
        }
        let re = Regex::new(r"\n{3,}").expect("whitespace pattern");
        let result = re.replace_all(&result, "\n\n");
        result.trim().to_string()
    }

    /// Apply all cleaning operations.
    pub fn clean_content(&self, text: &str) -> String {
        let text = self.strip_protocol_markers(text);
        let text = self.strip_system_noise(&text);
        text.trim().to_string()
    }

    /// Remove duplicate consecutive messages.
    pub fn dedupe_messages(&self, entries: &[ConversationEntry]) -> Vec<ConversationEntry> {
        if entries.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut prev_hash: Option<String> = None;

        for entry in entries {
            let content_hash = self.content_hash(entry);
            if Some(&content_hash) != prev_hash.as_ref() {
                result.push(entry.clone());
                prev_hash = Some(content_hash);
            }
        }

        result
    }

    /// Collapse consecutive tool calls into summaries.
    pub fn collapse_tool_calls(&self, entries: &[ConversationEntry]) -> Vec<ConversationEntry> {
        if entries.is_empty() {
            return Vec::new();
        }

        entries
            .iter()
            .map(|entry| {
                if entry.role == "assistant" && !entry.tool_calls.is_empty() {
                    self.collapse_tool_entry(entry)
                } else {
                    entry.clone()
                }
            })
            .collect()
    }

    fn matches_protocol_marker(&self, line: &str) -> bool {
        self.protocol_re.iter().any(|re| re.is_match(line))
    }

    fn normalize_for_hash(&self, text: &str) -> String {
        let re = Regex::new(r"\s+").expect("whitespace pattern");
        re.replace_all(text, " ").trim().to_lowercase()
    }

    fn content_hash(&self, entry: &ConversationEntry) -> String {
        let normalized = self.normalize_for_hash(&entry.content);
        format!("{}:{}", entry.role, hash_string(&normalized))
    }

    fn collapse_tool_entry(&self, entry: &ConversationEntry) -> ConversationEntry {
        let summary = self.summarize_tools(&entry.tool_calls);
        let content = append_tool_summary(&entry.content, &summary);
        ConversationEntry {
            role: entry.role.clone(),
            content,
            uuid: entry.uuid.clone(),
            parent_uuid: entry.parent_uuid.clone(),
            timestamp: entry.timestamp.clone(),
            tool_calls: Vec::new(),
        }
    }

    fn summarize_tools(&self, tool_calls: &[serde_json::Value]) -> String {
        if tool_calls.is_empty() {
            return String::new();
        }

        let grouped = group_tool_calls(tool_calls);
        grouped
            .iter()
            .map(|(name, calls)| summarize_tool_group(name, calls))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn append_tool_summary(content: &str, summary: &str) -> String {
    if content.is_empty() {
        format!("[Tools: {summary}]")
    } else {
        format!("{content}\n\n[Tools: {summary}]")
    }
}

fn group_tool_calls(
    tool_calls: &[serde_json::Value],
) -> std::collections::BTreeMap<String, Vec<&serde_json::Value>> {
    let mut by_name: std::collections::BTreeMap<String, Vec<&serde_json::Value>> =
        std::collections::BTreeMap::new();
    for tool_call in tool_calls {
        let name = tool_call
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        by_name.entry(name).or_default().push(tool_call);
    }
    by_name
}

fn summarize_tool_group(name: &str, calls: &[&serde_json::Value]) -> String {
    match name {
        "Read" | "Glob" | "Grep" => {
            summarize_file_tool_group(name, calls, &["file_path", "path", "pattern"])
        }
        "Edit" | "Write" => summarize_file_tool_group(name, calls, &["file_path"]),
        "Bash" => format!("Bash {} command(s)", calls.len()),
        _ => format!("{} x{}", name, calls.len()),
    }
}

fn summarize_file_tool_group(name: &str, calls: &[&serde_json::Value], keys: &[&str]) -> String {
    let files: Vec<String> = calls
        .iter()
        .filter_map(|tc| tool_basename(tc, keys))
        .filter(|s| !s.is_empty())
        .collect();

    if files.is_empty() {
        return format!("{} {} file(s)", name, calls.len());
    }

    let preview: Vec<String> = files.iter().take(3).cloned().collect();
    format!("{} {} file(s): {}", name, calls.len(), preview.join(", "))
}

fn tool_basename(tool_call: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let input = tool_call.get("input")?;
    if let Some(obj) = input.as_object() {
        for key in keys {
            if let Some(value) = obj.get(*key).and_then(|v| v.as_str()) {
                if !value.is_empty() {
                    return value.split('/').next_back().map(|s| s.to_string());
                }
            }
        }
    }
    None
}

fn hash_string(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
