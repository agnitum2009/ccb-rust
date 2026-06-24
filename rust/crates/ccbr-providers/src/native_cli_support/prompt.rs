use regex::Regex;

use ccbr_provider_core::protocol;

const ASSISTANT_UI_PREFIX_RE: &str = r"^•\s+";

/// Wrap a prompt with native CLI request guidance.
///
/// Mirrors Python `provider_backends.native_cli_support.prompt.wrap_native_prompt`.
pub fn wrap_native_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    format!(
        "{} {}\n\n{}\n\nCCBR reply guidance:\n\
         - Answer directly and concisely.\n\
         - Include only relevant conclusions, blockers, risks, evidence, and next actions.\n\
         - Avoid raw logs and background unless explicitly requested.\n",
        protocol::REQ_ID_PREFIX,
        req_id,
        rendered
    )
}

/// Clean a native CLI reply by removing markers and normalizing whitespace.
///
/// Mirrors Python `provider_backends.native_cli_support.prompt.clean_native_reply`.
pub fn clean_native_reply(text: &str, req_id: &str) -> String {
    let mut cleaned = text.replace("\r\n", "\n").replace('\r', "\n");
    if !req_id.is_empty() {
        cleaned = protocol::strip_done_text(&cleaned);
    }
    let re = Regex::new(protocol::ANY_DONE_LINE_RE).unwrap();
    cleaned = re.replace_all(&cleaned, "").to_string();
    let mut lines: Vec<String> = cleaned.split('\n').map(|line| line.rstrip()).collect();
    while !lines.is_empty() && lines[0].trim().is_empty() {
        lines.remove(0);
    }
    while !lines.is_empty() && lines.last().unwrap().trim().is_empty() {
        lines.pop();
    }
    let prefix_re = Regex::new(ASSISTANT_UI_PREFIX_RE).unwrap();
    for line in &mut lines {
        if !line.trim().is_empty() {
            *line = prefix_re.replace(line, "").to_string();
            break;
        }
    }
    lines.join("\n").trim().to_string()
}

trait Rstrip {
    fn rstrip(&self) -> String;
}

impl Rstrip for str {
    fn rstrip(&self) -> String {
        self.trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_native_prompt_format() {
        let wrapped = wrap_native_prompt("do the thing", "req-12345678");
        assert!(wrapped.contains("req-12345678"));
        assert!(wrapped.contains("do the thing"));
        assert!(wrapped.contains("CCBR reply guidance:"));
    }

    #[test]
    fn test_clean_native_reply_strips_markers() {
        let text = "\n\nhello world\n<<DONE:req-12345678>>\n";
        let cleaned = clean_native_reply(text, "req-12345678");
        assert_eq!(cleaned, "hello world");
    }

    #[test]
    fn test_clean_native_reply_strips_bullet_prefix() {
        let text = "• hello";
        assert_eq!(clean_native_reply(text, ""), "hello");
    }
}
