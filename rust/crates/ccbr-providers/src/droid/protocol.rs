use regex::Regex;
use std::sync::OnceLock;

pub const REQ_ID_PREFIX: &str = "CCBR_REQ_ID:";
pub const DONE_PREFIX: &str = "CCBR_DONE:";
pub const BEGIN_PREFIX: &str = "CCBR_BEGIN:";

/// Wrap a user message with the Droid request anchor and done marker instructions.
///
/// Mirrors Python `provider_backends.droid.protocol_runtime.prompt.wrap_droid_prompt`.
pub fn wrap_droid_prompt(message: &str, req_id: &str) -> String {
    let body = message.trim_end();
    format!(
        "{REQ_ID_PREFIX} {req_id}\n\n{body}\n\nIMPORTANT:\n\
         - Reply with an execution summary, in English. Do not stay silent.\n\
         - End your reply with this exact final line (verbatim, on its own line):\n{DONE_PREFIX} {req_id}\n"
    )
}

/// Check whether `text` ends with a done marker for the given request id.
///
/// Mirrors Python `provider_core.protocol_runtime.reply_runtime.markers.is_done_text`.
pub fn is_done_text(text: &str, req_id: &str) -> bool {
    let lines: Vec<_> = text.lines().map(|l| l.rstrip()).collect();
    for line in lines.iter().rev() {
        if is_trailing_noise_line(line) {
            continue;
        }
        return done_line_re(req_id).is_match(line);
    }
    false
}

/// Strip the trailing done marker for `req_id` from `text`.
///
/// Mirrors Python `provider_core.protocol_runtime.reply_runtime.markers.strip_done_text`.
pub fn strip_done_text(text: &str, req_id: &str) -> String {
    let mut lines: Vec<_> = split_lines(text);
    while let Some(last) = lines.last() {
        if is_trailing_noise_line(last) {
            lines.pop();
        } else {
            break;
        }
    }
    if let Some(last) = lines.last() {
        if done_line_re(req_id).is_match(last) {
            lines.pop();
        }
    }
    while let Some(last) = lines.last() {
        if is_trailing_noise_line(last) {
            lines.pop();
        } else {
            break;
        }
    }
    lines.join("\n").trim_end().to_string()
}

/// Extract the reply window belonging to the latest `CCBR_DONE:` marker for `req_id`.
///
/// Mirrors Python `provider_core.protocol_runtime.reply_runtime.extraction.extract_reply_for_req`.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    let lines = split_lines(text);
    if lines.is_empty() {
        return String::new();
    }
    let done_indexes = done_line_indexes(&lines);
    let target_indexes = target_done_indexes(&lines, req_id, &done_indexes);
    if target_indexes.is_empty() {
        return if done_indexes.is_empty() {
            strip_done_text(text, req_id)
        } else {
            String::new()
        };
    }
    extract_reply_window(&lines, &done_indexes, *target_indexes.last().unwrap(), None)
}

fn done_line_re(req_id: &str) -> Regex {
    let escaped = regex::escape(req_id);
    Regex::new(&format!(r"^\s*CCBR_DONE:\s*{escaped}\s*$")).unwrap()
}

fn any_done_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*CCBR_DONE:\s*(?:job_[a-z0-9]+|[0-9a-fA-F]{32}|\d{8}-\d{6}-\d{3}-\d+-\d+)\s*$",
        )
        .unwrap()
    })
}

fn done_line_indexes(lines: &[String]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| any_done_line_re().is_match(line))
        .map(|(i, _)| i)
        .collect()
}

fn target_done_indexes(lines: &[String], req_id: &str, done_indexes: &[usize]) -> Vec<usize> {
    let re = done_line_re(req_id);
    done_indexes
        .iter()
        .filter(|&&index| re.is_match(&lines[index]))
        .copied()
        .collect()
}

fn extract_reply_window(
    lines: &[String],
    done_indexes: &[usize],
    target_index: usize,
    start_index: Option<usize>,
) -> String {
    let start = start_index.unwrap_or_else(|| previous_done_index(done_indexes, target_index) + 1);
    trim_blank_edges(&lines[start..target_index])
        .join("\n")
        .trim_end()
        .to_string()
}

fn previous_done_index(done_indexes: &[usize], target_index: usize) -> usize {
    done_indexes
        .iter()
        .rev()
        .find(|&&index| index < target_index)
        .copied()
        .unwrap_or(usize::MAX)
}

fn trim_blank_edges(lines: &[String]) -> Vec<&str> {
    let mut start = 0;
    let mut end = lines.len();
    while start < end && lines[start].trim().is_empty() {
        start += 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[start..end].iter().map(|s| s.as_str()).collect()
}

fn is_trailing_noise_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    static TRAILING_DONE_TAG_RE: OnceLock<Regex> = OnceLock::new();
    let re = TRAILING_DONE_TAG_RE.get_or_init(|| {
        Regex::new(
            r"^\s*[A-Z][A-Z0-9_]*_DONE(?:\s*:\s*(?:job_[a-z0-9]+|[0-9a-fA-F]{32}|\d{8}-\d{6}-\d{3}-\d+-\d+))?\s*$",
        )
        .unwrap()
    });
    re.is_match(trimmed) && !trimmed.to_uppercase().starts_with("CCBR_DONE")
}

fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.rstrip().to_string()).collect()
}

trait Rstrip {
    fn rstrip(&self) -> &str;
}

impl Rstrip for str {
    fn rstrip(&self) -> &str {
        self.trim_end_matches(|c: char| c.is_whitespace())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_droid_prompt() {
        let out = wrap_droid_prompt("hello", "abc123");
        assert!(out.contains("CCBR_REQ_ID: abc123"));
        assert!(out.contains("CCBR_DONE: abc123"));
        assert!(out.contains("hello"));
    }

    #[test]
    fn test_is_done_text_true() {
        let text = "some reply\nCCBR_DONE: req-1";
        assert!(is_done_text(text, "req-1"));
    }

    #[test]
    fn test_is_done_text_false() {
        assert!(!is_done_text("some reply", "req-1"));
    }

    #[test]
    fn test_extract_reply_for_req() {
        // raw_buffer contains only assistant text in the Droid adapter.
        let text = "reply body\nCCBR_DONE: req-1";
        assert_eq!(extract_reply_for_req(text, "req-1"), "reply body");
    }

    #[test]
    fn test_strip_done_text() {
        let text = "reply body\nCCBR_DONE: req-1";
        assert_eq!(strip_done_text(text, "req-1"), "reply body");
    }
}
