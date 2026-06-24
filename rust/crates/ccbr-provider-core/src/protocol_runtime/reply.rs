use super::constants::{done_line_re, is_trailing_noise_line, ANY_DONE_LINE_RE};
use regex::RegexBuilder;

fn split_lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

fn trim_blank_edges<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut start = 0;
    let mut end = lines.len();
    while start < end && lines[start].trim().is_empty() {
        start += 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[start..end].to_vec()
}

fn previous_done_index(done_indexes: &[usize], target_index: usize) -> isize {
    done_indexes
        .iter()
        .rev()
        .find(|&&idx| idx < target_index)
        .copied()
        .map(|idx| idx as isize)
        .unwrap_or(-1)
}

fn done_line_indexes(lines: &[&str]) -> Vec<usize> {
    let re = RegexBuilder::new(ANY_DONE_LINE_RE)
        .case_insensitive(true)
        .build()
        .expect("done-line regex should compile");
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| re.is_match(line))
        .map(|(idx, _)| idx)
        .collect()
}

fn target_done_indexes(lines: &[&str], req_id: &str, done_indexes: Option<&[usize]>) -> Vec<usize> {
    let indexes = done_indexes
        .map(|d| d.to_vec())
        .unwrap_or_else(|| done_line_indexes(lines));
    let target_re = done_line_re(req_id);
    indexes
        .into_iter()
        .filter(|&idx| target_re.is_match(lines[idx]))
        .collect()
}

fn extract_reply_window(
    lines: &[&str],
    done_indexes: &[usize],
    target_index: usize,
    start_index: Option<usize>,
) -> String {
    let start = start_index
        .unwrap_or_else(|| (previous_done_index(done_indexes, target_index) + 1) as usize);
    let segment = trim_blank_edges(&lines[start..target_index]);
    segment.join("\n").trim_end().to_string()
}

/// Extract the reply text for a specific request ID from a provider response.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    let lines = split_lines(text);
    if lines.is_empty() {
        return String::new();
    }
    let done_indexes = done_line_indexes(&lines);
    let target_indexes = target_done_indexes(&lines, req_id, Some(&done_indexes));
    if target_indexes.is_empty() {
        return if done_indexes.is_empty() {
            strip_done_text(text, req_id)
        } else {
            String::new()
        };
    }
    extract_reply_window(
        &lines,
        &done_indexes,
        target_indexes[target_indexes.len() - 1],
        None,
    )
}

/// Return true if the final non-noise line is a done marker for `req_id`.
pub fn is_done_text(text: &str, req_id: &str) -> bool {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end()).collect();
    for line in lines.iter().rev() {
        if is_trailing_noise_line(line) {
            continue;
        }
        return done_line_re(req_id).is_match(line);
    }
    false
}

/// Remove a trailing done marker (and surrounding noise) for `req_id`.
pub fn strip_done_text(text: &str, req_id: &str) -> String {
    let mut lines: Vec<&str> = split_lines(text);
    if lines.is_empty() {
        return String::new();
    }
    while !lines.is_empty() && is_trailing_noise_line(lines[lines.len() - 1]) {
        lines.pop();
    }
    let target_re = done_line_re(req_id);
    if !lines.is_empty() && target_re.is_match(lines[lines.len() - 1]) {
        lines.pop();
    }
    while !lines.is_empty() && is_trailing_noise_line(lines[lines.len() - 1]) {
        lines.pop();
    }
    lines.join("\n").trim_end().to_string()
}

/// Remove trailing done/noise lines from a provider response.
pub fn strip_trailing_markers(text: &str) -> String {
    let mut lines: Vec<&str> = split_lines(text);
    let any_done_re = RegexBuilder::new(ANY_DONE_LINE_RE)
        .case_insensitive(true)
        .build()
        .expect("done-line regex should compile");
    while let Some(last) = lines.last() {
        if is_trailing_noise_line(last) || any_done_re.is_match(last) {
            lines.pop();
        } else {
            break;
        }
    }
    lines.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_reply_for_req() {
        let text = "CCBR_REQ_ID: job_abc123\n\nhello world\nCCBR_DONE: job_abc123";
        let reply = extract_reply_for_req(text, "job_abc123");
        assert_eq!(reply, "CCBR_REQ_ID: job_abc123\n\nhello world");
    }

    #[test]
    fn test_extract_reply_for_req_with_previous_done() {
        let text = "first\nCCBR_DONE: job_old\nsecond\nCCBR_DONE: job_new";
        let reply = extract_reply_for_req(text, "job_new");
        assert_eq!(reply, "second");
    }

    #[test]
    fn test_extract_reply_for_req_unknown_returns_empty() {
        let text = "hello\nCCBR_DONE: job_known";
        assert_eq!(extract_reply_for_req(text, "job_unknown"), "");
    }

    #[test]
    fn test_is_done_text() {
        assert!(is_done_text("reply\nCCBR_DONE: job_abc123", "job_abc123"));
        assert!(is_done_text(
            "reply\nnoise\n  ccbr_done: job_abc123  ",
            "job_abc123"
        ));
        assert!(!is_done_text("reply\nCCBR_DONE: job_def", "job_abc123"));
    }

    #[test]
    fn test_strip_done_text() {
        let text = "hello\n\nCCBR_DONE: job_abc123\n";
        assert_eq!(strip_done_text(text, "job_abc123"), "hello");
    }

    #[test]
    fn test_strip_trailing_markers() {
        let text = "hello\nCLAUDE_DONE:\nCCBR_DONE: job_abc123";
        assert_eq!(strip_trailing_markers(text), "hello");
    }

    #[test]
    fn test_strip_done_text_no_marker() {
        let text = "hello world";
        assert_eq!(strip_done_text(text, "job_abc123"), "hello world");
    }
}
