//! Mirrors Python `lib/provider_backends/pane_quiet_support/protocol.py`.

use regex::Regex;

use ccb_provider_core::protocol_runtime::{
    strip_done_text as strip_done_text_for_req, DONE_PREFIX, REQ_ID_PREFIX,
};

/// Wrap a pane-quiet prompt with request anchor and done marker instructions.
///
/// Mirrors Python `wrap_pane_quiet_prompt`.
pub fn wrap_pane_quiet_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    format!(
        "{REQ_ID_PREFIX} {req_id}\n\n{rendered}\n\n\
         IMPORTANT: when you finish answering, write this exact line on its \
         own line as the final line of your reply (no quoting, no code fence):\n\
         {DONE_PREFIX} {req_id}\n"
    )
}

/// Return true if `text` contains a request anchor for `req_id`.
///
/// Mirrors Python `pane_contains_req_anchor`.
pub fn pane_contains_req_anchor(text: &str, req_id: &str) -> bool {
    if text.is_empty() || req_id.is_empty() {
        return false;
    }
    req_anchor_re(req_id).is_match(text)
}

/// Extract the assistant reply for a request ID and report whether a done
/// marker was observed.
///
/// Mirrors Python `extract_reply_for_req`.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> (String, bool) {
    if text.is_empty() || req_id.is_empty() {
        return (String::new(), false);
    }

    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let anchor_matches: Vec<_> = req_anchor_re(req_id).find_iter(&text).collect();
    if anchor_matches.is_empty() {
        return (String::new(), false);
    }

    let after_anchor = &text[anchor_matches[anchor_matches.len() - 1].end()..];
    let done_matches: Vec<_> = done_anywhere_re(req_id).find_iter(after_anchor).collect();
    if done_matches.is_empty() {
        return (String::new(), false);
    }

    let body = if done_matches.len() == 1 {
        let body = &after_anchor[..done_matches[0].start()];
        if contains_banner_fragment(body) {
            return (String::new(), false);
        }
        body.to_string()
    } else {
        let echo_line_end = line_end(after_anchor, done_matches[done_matches.len() - 2].start());
        let model_line_start = line_start(after_anchor, done_matches[done_matches.len() - 1].start());
        let reply_start = if echo_line_end < after_anchor.len() {
            echo_line_end + 1
        } else {
            echo_line_end
        };
        after_anchor[reply_start..model_line_start].to_string()
    };

    let cleaned = clean_body(&body, req_id);
    if contains_banner_fragment(&cleaned) {
        (String::new(), false)
    } else {
        (cleaned, true)
    }
}

fn req_anchor_re(req_id: &str) -> Regex {
    Regex::new(&format!(
        r"{}\s*{}" ,
        regex::escape(REQ_ID_PREFIX),
        regex::escape(req_id)
    ))
    .unwrap()
}

fn done_anywhere_re(req_id: &str) -> Regex {
    Regex::new(&format!(
        r"{}\s*{}" ,
        regex::escape(DONE_PREFIX),
        regex::escape(req_id)
    ))
    .unwrap()
}

fn line_start(text: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end(text: &str, pos: usize) -> usize {
    pos + text[pos..].find('\n').unwrap_or(text.len() - pos)
}

fn contains_banner_fragment(text: &str) -> bool {
    let blob = text;
    for marker in BANNER_INSTRUCTIONS {
        if blob.contains(marker) {
            return true;
        }
    }
    for marker in BANNER_KEYWORDS {
        if blob.contains(marker) {
            return true;
        }
    }
    false
}

fn clean_body(body: &str, req_id: &str) -> String {
    let mut text = body.replace("\r\n", "\n").replace('\r', "\n");
    text = strip_done_text_for_req(&text, req_id);
    let any_done_re = Regex::new(ccb_provider_core::protocol_runtime::ANY_DONE_LINE_RE).unwrap();
    text = any_done_re.replace_all(&text, "").into_owned();

    let line_prefix = line_prefix_re();
    let assistant_prefix = assistant_ui_prefix_re();
    let mut cleaned_lines = Vec::new();
    for raw in text.split('\n') {
        let stripped = line_prefix.replace(raw, "").into_owned();
        let stripped = stripped.trim_end().to_string();
        if is_banner_line(&stripped) {
            continue;
        }
        cleaned_lines.push(stripped);
    }

    while let Some(first) = cleaned_lines.first() {
        if first.trim().is_empty() {
            cleaned_lines.remove(0);
        } else {
            break;
        }
    }
    while let Some(last) = cleaned_lines.last() {
        if last.trim().is_empty() {
            cleaned_lines.pop();
        } else {
            break;
        }
    }

    for line in cleaned_lines.iter_mut() {
        if !line.trim().is_empty() {
            *line = assistant_prefix.replace(line, "").into_owned();
            break;
        }
    }

    cleaned_lines.join("\n").trim().to_string()
}

fn is_banner_line(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty() {
        return false;
    }
    for marker in BANNER_KEYWORDS.iter().chain(BANNER_INSTRUCTIONS.iter()) {
        if text.contains(marker) {
            return true;
        }
    }
    false
}

fn line_prefix_re() -> Regex {
    Regex::new(r"^[\s>$#]+").unwrap()
}

fn assistant_ui_prefix_re() -> Regex {
    Regex::new(r"^•\s+").unwrap()
}

const BANNER_KEYWORDS: &[&str] = &["CCB_REQ_ID:", "CCB_DONE:"];
const BANNER_INSTRUCTIONS: &[&str] = &[
    "IMPORTANT: when you finish",
    "IMPORTANT:",
    "on its own line as the final line",
    "no quoting, no code fence",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_reply_for_req_handles_echo_and_model_done_markers() {
        let text = "CCB_REQ_ID: job_native123\nIMPORTANT: when you finish answering\nCCB_DONE: job_native123\nfinal answer\nCCB_DONE: job_native123\n";
        let (reply, done_seen) = extract_reply_for_req(text, "job_native123");
        assert!(done_seen);
        assert_eq!(reply, "final answer");
    }

    #[test]
    fn test_extract_reply_for_req_strips_kimi_tui_assistant_bullet() {
        let text = "CCB_REQ_ID: job_native123\nIMPORTANT: when you finish answering\nCCB_DONE: job_native123\n• final answer\n  CCB_DONE: job_native123\n";
        let (reply, done_seen) = extract_reply_for_req(text, "job_native123");
        assert!(done_seen);
        assert_eq!(reply, "final answer");
    }

    #[test]
    fn test_extract_reply_for_req_handles_single_model_done_marker_when_prompt_echo_is_hidden() {
        let text = "CCB_REQ_ID: job_native123\nfinal answer\nCCB_DONE: job_native123\n";
        let (reply, done_seen) = extract_reply_for_req(text, "job_native123");
        assert!(done_seen);
        assert_eq!(reply, "final answer");
    }

    #[test]
    fn test_extract_reply_for_req_ignores_single_prompt_echo_done_marker() {
        let text = "CCB_REQ_ID: job_native123\nplease answer\nIMPORTANT: when you finish answering, write this exact line\nCCB_DONE: job_native123\n";
        let (reply, done_seen) = extract_reply_for_req(text, "job_native123");
        assert!(!done_seen);
        assert_eq!(reply, "");
    }
}
