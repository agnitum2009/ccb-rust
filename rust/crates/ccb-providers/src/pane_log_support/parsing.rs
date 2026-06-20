//! Mirrors Python `lib/provider_backends/pane_log_support/parsing.py`.

use regex::Regex;

fn ansi_escape_re() -> Regex {
    Regex::new(r"\x1b(?:\[[\x30-\x3f]*[\x20-\x2f]*[\x40-\x7e]|\].*?(?:\x07|\x1b\\)|[\x40-\x5f])").unwrap()
}

fn ccb_req_id_re() -> Regex {
    Regex::new(r"^\s*CCB_REQ_ID:\s*(\S+)\s*$").unwrap()
}

fn ccb_done_re() -> Regex {
    Regex::new(r"(?i)^\s*CCB_DONE:\s*req-[a-f0-9]{8}\s*$").unwrap()
}

/// Strip ANSI escape sequences from text.
pub fn strip_ansi(text: &str) -> String {
    ansi_escape_re().replace_all(text, "").into_owned()
}

/// Return true if the text contains CCB protocol markers.
pub fn has_protocol_markers(text: &str) -> bool {
    ccb_req_id_re().is_match(text) || ccb_done_re().is_match(text)
}

/// Extract assistant reply blocks from pane log text.
///
/// Mirrors Python `extract_assistant_blocks`.
pub fn extract_assistant_blocks(text: &str) -> Vec<String> {
    if !has_protocol_markers(text) {
        let stripped = text.trim();
        return if stripped.is_empty() {
            Vec::new()
        } else {
            vec![stripped.to_string()]
        };
    }

    conversation_segments(text)
        .into_iter()
        .map(|(_user, assistant)| assistant)
        .filter(|assistant| !assistant.is_empty())
        .collect()
}

/// Extract user/assistant conversation pairs from pane log text.
///
/// Mirrors Python `extract_conversation_pairs`.
pub fn extract_conversation_pairs(text: &str) -> Vec<(String, String)> {
    conversation_segments(text)
}

fn conversation_segments(text: &str) -> Vec<(String, String)> {
    let done_re = ccb_done_re();
    let done_positions: Vec<usize> = done_re
        .find_iter(text)
        .map(|m| m.start())
        .collect();
    let req_re = ccb_req_id_re();
    let mut pairs = Vec::new();
    let mut prev_end = 0;
    for req_match in req_re.find_iter(text) {
        let user_text = text[prev_end..req_match.start()].trim().to_string();
        let (assistant_text, next_end) =
            assistant_segment(text, req_match.end(), &done_positions);
        prev_end = next_end;
        pairs.push((user_text, assistant_text));
    }
    pairs
}

fn assistant_segment(
    text: &str,
    req_end: usize,
    done_positions: &[usize],
) -> (String, usize) {
    match next_done_position(done_positions, req_end) {
        Some(next_done) => (
            text[req_end..next_done].trim().to_string(),
            next_done,
        ),
        None => (text[req_end..].trim().to_string(), text.len()),
    }
}

fn next_done_position(done_positions: &[usize], req_end: usize) -> Option<usize> {
    done_positions.iter().copied().find(|&pos| pos > req_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_assistant_blocks_returns_plain_text_without_protocol_markers() {
        assert_eq!(
            extract_assistant_blocks("  hello world  "),
            vec!["hello world".to_string()]
        );
    }

    #[test]
    fn test_extract_assistant_blocks_collects_each_protocol_segment() {
        let text = "user1\nCCB_REQ_ID: job_1\nassistant one\nCCB_DONE: req-12345678\nuser2\nCCB_REQ_ID: job_2\nassistant two\n";
        assert_eq!(
            extract_assistant_blocks(text),
            vec!["assistant one".to_string(), "assistant two".to_string()]
        );
    }

    #[test]
    fn test_extract_conversation_pairs_preserves_user_and_assistant_segments() {
        let text = "first user\nCCB_REQ_ID: job_1\nfirst assistant\nCCB_DONE: req-12345678\nsecond user\nCCB_REQ_ID: job_2\nsecond assistant\n";
        assert_eq!(
            extract_conversation_pairs(text),
            vec![
                ("first user".to_string(), "first assistant".to_string()),
                (
                    "CCB_DONE: req-12345678\nsecond user".to_string(),
                    "second assistant".to_string()
                ),
            ]
        );
    }
}
