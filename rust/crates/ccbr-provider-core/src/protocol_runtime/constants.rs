use regex::RegexBuilder;

/// Prefix that begins a request anchor marker.
pub const BEGIN_PREFIX: &str = "CCB_BEGIN:";

/// Prefix that signals a request is complete.
pub const DONE_PREFIX: &str = "CCB_DONE:";

/// Prefix placed before a request ID in a prompt.
pub const REQ_ID_PREFIX: &str = "CCB_REQ_ID:";

/// Pattern matching a legacy 32-character hex request ID.
pub const LEGACY_HEX_REQ_ID_PATTERN: &str = r"[0-9a-fA-F]{32}";

/// Pattern matching a legacy timestamp-style request ID.
pub const LEGACY_TIMESTAMP_REQ_ID_PATTERN: &str = r"\d{8}-\d{6}-\d{3}-\d+-\d+";

/// Pattern matching a job-derived request ID.
pub const JOB_REQ_ID_PATTERN: &str = r"job_[a-z0-9]+";

/// Pattern matching any supported request ID format.
pub const ANY_REQ_ID_PATTERN: &str = r"(?:job_[a-z0-9]+|[0-9a-fA-F]{32}|\d{8}-\d{6}-\d{3}-\d+-\d+)";

/// Boundary used after a request ID to avoid matching partial identifiers.
pub const REQ_ID_BOUNDARY_PATTERN: &str = r"(?=[^A-Za-z0-9_-]|$)";

/// Template for a done-line regex for a specific request ID.
pub const DONE_LINE_RE_TEMPLATE: &str = r"^\s*CCB_DONE:\s*{req_id}\s*$";

/// Regex pattern (as a string) matching any done line.
pub const ANY_DONE_LINE_RE: &str =
    r"(?i)^\s*CCB_DONE:\s*(?:job_[a-z0-9]+|[0-9a-fA-F]{32}|\d{8}-\d{6}-\d{3}-\d+-\d+)\s*$";

/// Build a case-insensitive regex matching the done line for `req_id`.
pub fn done_line_re(req_id: &str) -> regex::Regex {
    let pattern = DONE_LINE_RE_TEMPLATE.replace("{req_id}", &regex::escape(req_id));
    RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .expect("done-line regex should compile")
}

/// Return true for blank lines or provider-specific trailing done tags.
pub fn is_trailing_noise_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    // Match lines like `CLAUDE_DONE:` or `CLAUDE_DONE: <req_id>` but not `CCB_DONE:`.
    if trimmed.to_ascii_uppercase().starts_with("CCB_DONE:") {
        return false;
    }
    if let Some(pos) = trimmed.rfind("_DONE") {
        let prefix = &trimmed[..pos];
        let rest = &trimmed[pos + "_DONE".len()..];
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim();
        let prefix_ok = !prefix.is_empty()
            && prefix
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        let rest_ok = rest.is_empty()
            || regex::Regex::new(ANY_REQ_ID_PATTERN)
                .unwrap()
                .is_match(rest);
        return prefix_ok && rest_ok;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_match_expected_prefixes() {
        assert_eq!(BEGIN_PREFIX, "CCB_BEGIN:");
        assert_eq!(DONE_PREFIX, "CCB_DONE:");
        assert_eq!(REQ_ID_PREFIX, "CCB_REQ_ID:");
    }

    #[test]
    fn test_any_done_line_re_matches_variants() {
        let re = regex::Regex::new(ANY_DONE_LINE_RE).unwrap();
        assert!(re.is_match("CCB_DONE: job_abc123"));
        assert!(re.is_match("  ccbr_done:  job_abc123  "));
        assert!(re.is_match("CCB_DONE: 0123456789abcdef0123456789abcdef"));
        assert!(re.is_match("CCB_DONE: 20240102-030405-006-1234-1"));
        assert!(!re.is_match("CLAUDE_DONE: job_abc123"));
        assert!(!re.is_match("CCB_DONE:"));
    }

    #[test]
    fn test_done_line_re_matches_specific_req_id() {
        let re = done_line_re("job_abc123");
        assert!(re.is_match("CCB_DONE: job_abc123"));
        assert!(re.is_match("  ccbr_done: job_abc123  "));
        assert!(!re.is_match("CCB_DONE: job_def456"));
    }

    #[test]
    fn test_is_trailing_noise_line() {
        assert!(is_trailing_noise_line(""));
        assert!(is_trailing_noise_line("   "));
        assert!(is_trailing_noise_line("CLAUDE_DONE:"));
        assert!(is_trailing_noise_line("CLAUDE_DONE: job_abc123"));
        assert!(!is_trailing_noise_line("CCB_DONE: job_abc123"));
        assert!(!is_trailing_noise_line("hello"));
    }
}
