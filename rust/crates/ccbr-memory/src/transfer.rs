use crate::deduper::ConversationDeduper;
use crate::formatter::ContextFormatter;
use crate::session_parser::ClaudeSessionParser;
use crate::types::{ConversationEntry, MemoryError, Result, SessionStats, TransferContext};
use ccbr_provider_sessions::files::find_bound_session_file;
use ccbr_storage::path_helpers::normalize_agent_name;

use ccbr_types::env::env_bool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SUPPORTED_SOURCES: &[&str] = &["auto", "claude", "codex", "gemini", "opencode", "droid"];

pub fn source_session_files() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("claude".to_string(), ".claude-session".to_string());
    m.insert("codex".to_string(), ".codex-session".to_string());
    m.insert("gemini".to_string(), ".gemini-session".to_string());
    m.insert("opencode".to_string(), ".opencode-session".to_string());
    m.insert("droid".to_string(), ".droid-session".to_string());
    m
}

pub const DEFAULT_SOURCE_ORDER: &[&str] = &["claude", "codex", "gemini", "opencode", "droid"];
pub const DEFAULT_FALLBACK_PAIRS: usize = 50;

/// Orchestrate context transfer between providers.
#[derive(Debug, Clone)]
pub struct ContextTransfer {
    pub max_tokens: u32,
    pub work_dir: PathBuf,
    pub parser: ClaudeSessionParser,
    pub deduper: ConversationDeduper,
    pub formatter: ContextFormatter,
}

impl Default for ContextTransfer {
    fn default() -> Self {
        Self::new(
            8000,
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        )
    }
}

impl ContextTransfer {
    pub fn new(max_tokens: u32, work_dir: impl Into<PathBuf>) -> Self {
        Self {
            max_tokens,
            work_dir: work_dir.into(),
            parser: ClaudeSessionParser::default(),
            deduper: ConversationDeduper::new(),
            formatter: ContextFormatter::new(max_tokens),
        }
    }

    /// Extract and process conversations from a session.
    pub fn extract_conversations(
        &self,
        session_path: Option<&Path>,
        last_n: usize,
        include_stats: bool,
        source_provider: &str,
        source_session_id: Option<&str>,
        source_project_id: Option<&str>,
    ) -> Result<TransferContext> {
        let provider = normalize_provider(source_provider);

        if provider == "auto" {
            if session_path.is_some() {
                return self.extract_from_claude(session_path, last_n, include_stats);
            }

            let mut last_error: Option<MemoryError> = None;
            for candidate in self.auto_source_candidates() {
                match self.extract_by_provider(
                    &candidate,
                    session_path,
                    last_n,
                    include_stats,
                    source_session_id,
                    source_project_id,
                ) {
                    Ok(ctx) => return Ok(ctx),
                    Err(err) => {
                        last_error = Some(err);
                        continue;
                    }
                }
            }

            if let Some(err) = last_error {
                return Err(err);
            }
            return Err(MemoryError::SessionNotFound(
                "No sessions found for any provider".to_string(),
            ));
        }

        self.extract_by_provider(
            &provider,
            session_path,
            last_n,
            include_stats,
            source_session_id,
            source_project_id,
        )
    }

    /// Format context for output.
    pub fn format_output(&self, context: &TransferContext, fmt: &str, detailed: bool) -> String {
        self.formatter.format(context, fmt, detailed)
    }

    /// Save transfer output to the project history directory.
    pub fn save_transfer(
        &self,
        context: &TransferContext,
        fmt: &str,
        target_agent: Option<&str>,
        filename: Option<&str>,
    ) -> Result<PathBuf> {
        let history_dir = history_dir(&self.work_dir);
        std::fs::create_dir_all(&history_dir)?;

        let ext = match fmt {
            "plain" => "txt",
            "json" => "json",
            _ => "md",
        };

        let filepath = if let Some(name) = filename {
            let safe = name.trim().replace(['/', '\\'], "-");
            let path = history_dir.join(&safe);
            if path.extension().is_none() {
                history_dir.join(format!("{safe}.{ext}"))
            } else {
                path
            }
        } else {
            let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
            let session_short = if context.source_session_id.len() >= 8 {
                &context.source_session_id[..8]
            } else {
                &context.source_session_id
            };
            let source_provider = if context.source_provider.is_empty() {
                "session"
            } else {
                context.source_provider.trim()
            }
            .to_lowercase()
            .replace(['/', '\\'], "-");
            let provider_suffix = target_agent
                .map(|a| format!("-to-{}", normalize_name(a)))
                .unwrap_or_default();
            history_dir.join(format!(
                "{source_provider}-{ts}-{session_short}{provider_suffix}.{ext}"
            ))
        };

        let formatted = self.format_output(context, fmt, false);
        std::fs::write(&filepath, formatted)?;
        Ok(filepath)
    }

    fn auto_source_candidates(&self) -> Vec<String> {
        auto_source_candidates(
            &self.work_dir,
            DEFAULT_SOURCE_ORDER,
            &source_session_files(),
        )
    }

    fn extract_by_provider(
        &self,
        provider: &str,
        session_path: Option<&Path>,
        last_n: usize,
        include_stats: bool,
        source_session_id: Option<&str>,
        source_project_id: Option<&str>,
    ) -> Result<TransferContext> {
        match provider {
            "claude" => self.extract_from_claude(session_path, last_n, include_stats),
            "codex" => self.extract_from_codex(session_path, source_session_id, last_n),
            "gemini" => self.extract_from_gemini(session_path, source_session_id, last_n),
            "droid" => self.extract_from_droid(session_path, source_session_id, last_n),
            "opencode" => self.extract_from_opencode(source_session_id, source_project_id, last_n),
            _ => Err(MemoryError::SessionNotFound(format!(
                "Unsupported source provider: {provider}"
            ))),
        }
    }

    fn extract_from_claude(
        &self,
        session_path: Option<&Path>,
        last_n: usize,
        include_stats: bool,
    ) -> Result<TransferContext> {
        let resolved = self.parser.resolve_session(&self.work_dir, session_path)?;
        let mut info = self.parser.get_session_info(&resolved)?;
        info.provider = Some("claude".to_string());

        let stats = if include_stats {
            Some(self.parser.extract_session_stats(&resolved)?)
        } else {
            None
        };

        let mut entries = self.parser.parse_session(&resolved)?;
        entries = clean_entries(&self.deduper, &entries);
        entries = self.deduper.dedupe_messages(&entries);
        entries = self.deduper.collapse_tool_calls(&entries);

        let mut pairs = build_pairs(&entries);
        if last_n > 0 && pairs.len() > last_n {
            pairs = pairs.split_off(pairs.len() - last_n);
        }

        let pairs = self
            .formatter
            .truncate_to_limit(&pairs, Some(self.max_tokens));
        let total_text: String = pairs.iter().map(|(u, a)| format!("{u}{a}")).collect();
        let token_estimate = self.formatter.estimate_tokens(&total_text);

        Ok(TransferContext {
            conversations: pairs,
            source_session_id: info.session_id,
            token_estimate,
            metadata: serde_json::json!({
                "session_path": resolved.to_string_lossy().to_string(),
                "provider": "claude",
            }),
            stats,
            source_provider: "claude".to_string(),
        })
    }

    fn extract_from_codex(
        &self,
        session_path: Option<&Path>,
        session_id: Option<&str>,
        last_n: usize,
    ) -> Result<TransferContext> {
        extract_from_provider_session(
            &self.work_dir,
            "codex",
            session_path,
            session_id,
            last_n,
            self.max_tokens,
            &self.deduper,
            &self.formatter,
        )
    }

    fn extract_from_gemini(
        &self,
        session_path: Option<&Path>,
        session_id: Option<&str>,
        last_n: usize,
    ) -> Result<TransferContext> {
        extract_from_provider_session(
            &self.work_dir,
            "gemini",
            session_path,
            session_id,
            last_n,
            self.max_tokens,
            &self.deduper,
            &self.formatter,
        )
    }

    fn extract_from_droid(
        &self,
        session_path: Option<&Path>,
        session_id: Option<&str>,
        last_n: usize,
    ) -> Result<TransferContext> {
        extract_from_provider_session(
            &self.work_dir,
            "droid",
            session_path,
            session_id,
            last_n,
            self.max_tokens,
            &self.deduper,
            &self.formatter,
        )
    }

    fn extract_from_opencode(
        &self,
        session_id: Option<&str>,
        project_id: Option<&str>,
        last_n: usize,
    ) -> Result<TransferContext> {
        let _ = (session_id, project_id, last_n);
        Err(MemoryError::SessionNotFound(
            "OpenCode provider backend is not available in the Rust migration".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Common pipeline helpers
// ---------------------------------------------------------------------------

pub fn clean_entries(
    deduper: &ConversationDeduper,
    entries: &[ConversationEntry],
) -> Vec<ConversationEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            let cleaned = deduper.clean_content(&entry.content);
            if cleaned.is_empty() && entry.tool_calls.is_empty() {
                return None;
            }
            Some(ConversationEntry {
                role: entry.role.clone(),
                content: cleaned,
                uuid: entry.uuid.clone(),
                parent_uuid: entry.parent_uuid.clone(),
                timestamp: entry.timestamp.clone(),
                tool_calls: entry.tool_calls.clone(),
            })
        })
        .collect()
}

pub fn build_pairs(entries: &[ConversationEntry]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_user: Option<String> = None;

    for entry in entries {
        if entry.role == "user" {
            current_user = Some(entry.content.clone());
        } else if entry.role == "assistant" && current_user.is_some() {
            pairs.push((current_user.take().unwrap(), entry.content.clone()));
        }
    }

    pairs
}

#[allow(clippy::too_many_arguments)]
pub fn context_from_pairs(
    deduper: &ConversationDeduper,
    formatter: &ContextFormatter,
    max_tokens: u32,
    pairs: &[(String, String)],
    provider: &str,
    session_id: &str,
    session_path: Option<&Path>,
    last_n: usize,
    stats: Option<SessionStats>,
) -> TransferContext {
    let mut cleaned_pairs: Vec<(String, String)> = Vec::new();
    let mut prev_hash: Option<String> = None;

    for (user_msg, assistant_msg) in pairs {
        let cleaned_user = deduper.clean_content(user_msg);
        let cleaned_assistant = deduper.clean_content(assistant_msg);
        if cleaned_user.is_empty() && cleaned_assistant.is_empty() {
            continue;
        }
        let pair_hash = format!(
            "{}::{}",
            hash_string(&cleaned_user),
            hash_string(&cleaned_assistant)
        );
        if Some(&pair_hash) == prev_hash.as_ref() {
            continue;
        }
        cleaned_pairs.push((cleaned_user, cleaned_assistant));
        prev_hash = Some(pair_hash);
    }

    if last_n > 0 && cleaned_pairs.len() > last_n {
        cleaned_pairs = cleaned_pairs.split_off(cleaned_pairs.len() - last_n);
    }

    let cleaned_pairs = formatter.truncate_to_limit(&cleaned_pairs, Some(max_tokens));
    let total_text: String = cleaned_pairs
        .iter()
        .map(|(u, a)| format!("{u}{a}"))
        .collect();
    let token_estimate = formatter.estimate_tokens(&total_text);

    let mut metadata = serde_json::json!({"provider": provider});
    if let Some(path) = session_path {
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "session_path".to_string(),
                serde_json::Value::String(path.to_string_lossy().to_string()),
            );
        }
    }

    TransferContext {
        conversations: cleaned_pairs,
        source_session_id: session_id.to_string(),
        token_estimate,
        metadata,
        stats,
        source_provider: provider.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Provider session file helpers
// ---------------------------------------------------------------------------

pub fn auto_source_candidates(
    work_dir: &Path,
    default_source_order: &[&str],
    source_session_files: &HashMap<String, String>,
) -> Vec<String> {
    let mut candidates: Vec<(f64, String)> = Vec::new();
    for provider in default_source_order {
        let Some(filename) = source_session_files.get(*provider) else {
            continue;
        };
        let session_file = find_bound_session_file(work_dir, provider, filename);
        if session_file.is_none() || !session_file.as_ref().unwrap().exists() {
            continue;
        }
        let mtime = session_file
            .as_ref()
            .unwrap()
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        candidates.push((mtime, (*provider).to_string()));
    }

    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut ordered: Vec<String> = candidates.into_iter().map(|(_, p)| p).collect();

    for provider in default_source_order {
        if !ordered.iter().any(|p| p == *provider) {
            ordered.push((*provider).to_string());
        }
    }
    ordered
}

pub fn load_session_data(
    work_dir: &Path,
    provider: &str,
) -> (Option<PathBuf>, serde_json::Map<String, serde_json::Value>) {
    let filename = source_session_files()
        .get(provider)
        .cloned()
        .unwrap_or_default();
    if filename.is_empty() {
        return (None, serde_json::Map::new());
    }
    let session_file = find_bound_session_file(work_dir, provider, &filename);
    let Some(session_file) = session_file else {
        return (None, serde_json::Map::new());
    };
    if !session_file.exists() {
        return (None, serde_json::Map::new());
    }

    let raw = match std::fs::read_to_string(&session_file) {
        Ok(text) => text,
        Err(_) => return (Some(session_file), serde_json::Map::new()),
    };

    let data: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return (Some(session_file), serde_json::Map::new()),
    };

    (
        Some(session_file),
        data.as_object().cloned().unwrap_or_default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn extract_from_provider_session(
    work_dir: &Path,
    provider: &str,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    last_n: usize,
    max_tokens: u32,
    deduper: &ConversationDeduper,
    formatter: &ContextFormatter,
) -> Result<TransferContext> {
    let (session_file, data) = load_session_data(work_dir, provider);

    let resolved_path = session_path
        .map(|p| p.to_path_buf())
        .or_else(|| {
            data.get(&format!("{provider}_session_path"))
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
        })
        .or(session_file)
        .filter(|p| p.exists());

    let resolved_session_id = session_id
        .map(|s| s.to_string())
        .or_else(|| {
            data.get(&format!("{provider}_session_id"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| {
            resolved_path
                .as_ref()
                .and_then(|p| p.file_stem().and_then(|s| s.to_str()).map(String::from))
                .unwrap_or_else(|| "unknown".to_string())
        });

    if resolved_path.is_none() {
        return Err(MemoryError::SessionNotFound(format!(
            "No {provider} session found"
        )));
    }

    // Attempt a generic conversation extraction from the session file JSON.
    let pairs = generic_pairs_from_file(resolved_path.as_ref().unwrap())?;
    if pairs.is_empty() {
        return Err(MemoryError::SessionNotFound(format!(
            "No {provider} conversation pairs found"
        )));
    }

    Ok(context_from_pairs(
        deduper,
        formatter,
        max_tokens,
        &pairs,
        provider,
        &resolved_session_id,
        resolved_path.as_deref(),
        if last_n > 0 {
            last_n
        } else {
            DEFAULT_FALLBACK_PAIRS
        },
        None,
    ))
}

fn generic_pairs_from_file(path: &Path) -> Result<Vec<(String, String)>> {
    let raw = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| MemoryError::SessionParse(format!("Failed to parse session JSON: {e}")))?;

    if let Some(pairs) = value.get("conversations").and_then(|v| v.as_array()) {
        return Ok(extract_pairs_from_array(pairs));
    }
    if let Some(messages) = value.get("messages").and_then(|v| v.as_array()) {
        return Ok(extract_pairs_from_messages(messages));
    }
    if let Some(arr) = value.as_array() {
        return Ok(extract_pairs_from_messages(arr));
    }
    Ok(Vec::new())
}

fn extract_pairs_from_array(pairs: &[serde_json::Value]) -> Vec<(String, String)> {
    pairs
        .iter()
        .filter_map(|p| {
            let obj = p.as_object()?;
            let user = obj
                .get("user")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("user_message").and_then(|v| v.as_str()))?
                .to_string();
            let assistant = obj
                .get("assistant")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("assistant_message").and_then(|v| v.as_str()))?
                .to_string();
            Some((user, assistant))
        })
        .collect()
}

fn extract_pairs_from_messages(messages: &[serde_json::Value]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_user: Option<String> = None;
    for msg in messages {
        let Some(obj) = msg.as_object() else { continue };
        let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let content = obj
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if role == "user" {
            current_user = Some(content);
        } else if role == "assistant" && current_user.is_some() {
            pairs.push((current_user.take().unwrap(), content));
        }
    }
    pairs
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

pub fn history_dir(work_dir: &Path) -> PathBuf {
    let resolved = match std::fs::canonicalize(work_dir) {
        Ok(p) => p,
        Err(_) => work_dir.to_path_buf(),
    };
    ccbr_provider_sessions::files::resolve_project_config_dir(&resolved).join("history")
}

fn normalize_provider(provider: &str) -> String {
    let value = provider.trim().to_lowercase();
    if value.is_empty() {
        "auto".to_string()
    } else {
        value
    }
}

fn normalize_name(name: &str) -> String {
    normalize_agent_name(name).unwrap_or_else(|_| name.trim().to_lowercase())
}

fn hash_string(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Auto-transfer helpers
// ---------------------------------------------------------------------------

/// Check whether an automatic context transfer should run for a session switch.
pub fn maybe_auto_transfer(
    provider: &str,
    work_dir: &Path,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    project_id: Option<&str>,
) {
    if !env_bool("CCB_CTX_TRANSFER_ON_SESSION_SWITCH", true) {
        return;
    }
    if session_path.is_none() && session_id.is_none() {
        return;
    }
    let normalized_work_dir = match std::fs::canonicalize(work_dir) {
        Ok(p) => p,
        Err(_) => work_dir.to_path_buf(),
    };
    if !is_current_work_dir(&normalized_work_dir) {
        return;
    }
    let key = auto_transfer_key(
        provider,
        &normalized_work_dir,
        session_path,
        session_id,
        project_id,
    );
    if !claim_auto_transfer(&key) {
        return;
    }
    // Synchronous best-effort run; Python uses a background thread.
    let _ = run_auto_transfer(
        provider,
        &normalized_work_dir,
        session_path,
        session_id,
        project_id,
    );
}

fn run_auto_transfer(
    provider: &str,
    work_dir: &Path,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<PathBuf> {
    let last_n = ccbr_types::env::env_int("CCB_CTX_TRANSFER_LAST_N", 3) as usize;
    let max_tokens = ccbr_types::env::env_int("CCB_CTX_TRANSFER_MAX_TOKENS", 8000) as u32;
    let fmt = normalized_env("CCB_CTX_TRANSFER_FORMAT", "markdown");
    let target_provider = normalized_env("CCB_CTX_TRANSFER_PROVIDER", "auto");

    let transfer = ContextTransfer::new(max_tokens, work_dir);
    let context = transfer.extract_conversations(
        session_path,
        last_n,
        true,
        provider,
        session_id,
        project_id,
    )?;
    if context.conversations.is_empty() {
        return Err(MemoryError::SessionNotFound(
            "No conversations to transfer".to_string(),
        ));
    }

    let filename = transfer_filename(provider, session_path, session_id);
    transfer.save_transfer(&context, &fmt, Some(&target_provider), Some(&filename))
}

fn auto_transfer_key(
    provider: &str,
    work_dir: &Path,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> String {
    format!(
        "{}::{}::{}::{}::{}",
        provider,
        work_dir.display(),
        session_path
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        session_id.unwrap_or(""),
        project_id.unwrap_or("")
    )
}

fn claim_auto_transfer(key: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEEN: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);
    const TTL_S: u64 = 3600;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut guard = SEEN.lock().unwrap();
    let seen = guard.get_or_insert_with(HashMap::new);

    if seen.contains_key(key) {
        return false;
    }

    seen.retain(|_, ts| now - *ts <= TTL_S);
    seen.insert(key.to_string(), now);
    true
}

fn is_current_work_dir(work_dir: &Path) -> bool {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    normalize_path_for_match(&cwd) == normalize_path_for_match(work_dir)
}

fn normalize_path_for_match(value: &Path) -> String {
    std::fs::canonicalize(value)
        .unwrap_or_else(|_| value.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn normalized_env(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn transfer_filename(
    provider: &str,
    session_path: Option<&Path>,
    session_id: Option<&str>,
) -> String {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let sid = session_id
        .map(|s| s.to_string())
        .or_else(|| {
            session_path.and_then(|p| p.file_stem().and_then(|s| s.to_str()).map(String::from))
        })
        .unwrap_or_else(|| "unknown".to_string());
    format!("{provider}-{ts}-{sid}")
}
