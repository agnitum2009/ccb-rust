use serde::{Deserialize, Serialize};

/// A request to a Codex-compatible provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRequest {
    pub client_id: String,
    pub work_dir: String,
    pub timeout_s: f64,
    pub quiet: bool,
    pub message: String,
    #[serde(default)]
    pub req_id: Option<String>,
    #[serde(default = "default_caller")]
    pub caller: String,
}

fn default_caller() -> String {
    "claude".to_string()
}

/// The result of a Codex request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexResult {
    pub exit_code: i32,
    pub reply: String,
    pub req_id: String,
    pub session_key: String,
    pub log_path: Option<String>,
    pub anchor_seen: bool,
    pub done_seen: bool,
    pub fallback_scan: bool,
    #[serde(default)]
    pub anchor_ms: Option<u64>,
    #[serde(default)]
    pub done_ms: Option<u64>,
}

/// Protocol constants.
pub const BEGIN_PREFIX: &str = "<<BEGIN:";
pub const DONE_PREFIX: &str = "<<DONE:";
pub const REQ_ID_PREFIX: &str = "req-";
pub const REQ_ID_BOUNDARY_PATTERN: &str = r"req-[a-f0-9]{8}";
pub const ANY_REQ_ID_PATTERN: &str = r"req-[a-f0-9]{8}";
pub const ANY_DONE_LINE_RE: &str = r"<<DONE:req-[a-f0-9]{8}>>";

/// Generate a request ID from a job ID.
pub fn make_req_id(job_id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    job_id.hash(&mut hasher);
    format!("req-{:08x}", (hasher.finish() & 0xFFFF_FFFF) as u32)
}

/// Create a request anchor marker.
pub fn request_anchor_for_job(job_id: &str) -> String {
    format!("{}{}>>", BEGIN_PREFIX, make_req_id(job_id))
}

/// Create a done marker.
pub fn done_marker(req_id: &str) -> String {
    format!("{}{}>>", DONE_PREFIX, req_id)
}

/// Check if text contains a done marker.
pub fn is_done_text(text: &str) -> bool {
    text.contains(DONE_PREFIX)
}

/// Strip done markers from text.
pub fn strip_done_text(text: &str) -> String {
    let re = regex::Regex::new(ANY_DONE_LINE_RE).unwrap();
    re.replace_all(text, "").to_string()
}

/// Strip trailing markers (begin/done) from text.
pub fn strip_trailing_markers(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(pos) = result.rfind("<<") {
        if result[pos..].contains(">>") {
            result.truncate(pos);
        } else {
            break;
        }
    }
    result.trim_end().to_string()
}

/// Extract reply text for a specific request ID.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    let begin_marker = format!("{}{}>>", BEGIN_PREFIX, req_id);
    let done_marker = format!("{}{}>>", DONE_PREFIX, req_id);

    let start = text
        .find(&begin_marker)
        .map(|p| p + begin_marker.len())
        .unwrap_or(0);
    let end = text.find(&done_marker).unwrap_or(text.len());

    if start <= end {
        text[start..end].trim().to_string()
    } else {
        String::new()
    }
}

/// Wrap a prompt with begin/done markers for a turn-based conversation.
pub fn wrap_codex_turn_prompt(message: &str, req_id: &str) -> String {
    format!(
        "{}{}>>\n{}\n{}{}>>",
        BEGIN_PREFIX, req_id, message, DONE_PREFIX, req_id
    )
}

/// Wrap a prompt with a begin marker only (for streaming).
pub fn wrap_codex_prompt(message: &str, req_id: &str) -> String {
    format!("{}{}>>\n{}", BEGIN_PREFIX, req_id, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_req_id_deterministic() {
        let a = make_req_id("job-123");
        let b = make_req_id("job-123");
        assert_eq!(a, b);
        assert!(a.starts_with("req-"));
        assert_eq!(a.len(), 12); // "req-" + 8 hex chars
    }

    #[test]
    fn test_request_anchor_for_job() {
        let anchor = request_anchor_for_job("job-1");
        assert!(anchor.starts_with("<<BEGIN:req-"));
        assert!(anchor.ends_with(">>"));
    }

    #[test]
    fn test_is_done_text() {
        assert!(is_done_text("some text <<DONE:req-12345678>> more"));
        assert!(!is_done_text("no done marker here"));
    }

    #[test]
    fn test_strip_done_text() {
        let text = "hello\n<<DONE:req-12345678>>\nworld";
        let stripped = strip_done_text(text);
        assert_eq!(stripped, "hello\n\nworld");
    }

    #[test]
    fn test_extract_reply_for_req() {
        let text = "<<BEGIN:req-12345678>>\nhello world\n<<DONE:req-12345678>>";
        let reply = extract_reply_for_req(text, "req-12345678");
        assert_eq!(reply, "hello world");
    }

    #[test]
    fn test_wrap_codex_turn_prompt() {
        let wrapped = wrap_codex_turn_prompt("hello", "req-abcd1234");
        assert!(wrapped.contains("<<BEGIN:req-abcd1234>>"));
        assert!(wrapped.contains("hello"));
        assert!(wrapped.contains("<<DONE:req-abcd1234>>"));
    }

    #[test]
    fn test_codex_request_serde() {
        let req = CodexRequest {
            client_id: "test".into(),
            work_dir: "/tmp".into(),
            timeout_s: 30.0,
            quiet: false,
            message: "hello".into(),
            req_id: None,
            caller: "claude".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: CodexRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.client_id, "test");
        assert_eq!(deserialized.caller, "claude");
    }
}
