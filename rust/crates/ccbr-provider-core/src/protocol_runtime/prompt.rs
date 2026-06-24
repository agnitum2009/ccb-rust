use super::constants::{DONE_PREFIX, REQ_ID_PREFIX};

/// Wrap a prompt with a request ID and a required final `CCB_DONE:` line.
pub fn wrap_codex_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    format!(
        "{} {req_id}\n\n{rendered}\n\nIMPORTANT:\n\
         - Reply normally.\n\
         - Reply normally, in English.\n\
         - End your reply with this exact final line (verbatim, on its own line):\n\
         {DONE_PREFIX} {req_id}\n",
        REQ_ID_PREFIX
    )
}

/// Wrap a prompt with a request ID for a streaming turn.
pub fn wrap_codex_turn_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    format!("{} {req_id}\n\n{rendered}\n", REQ_ID_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_codex_prompt() {
        let wrapped = wrap_codex_prompt("hello", "job_abc123");
        assert!(wrapped.contains("CCB_REQ_ID: job_abc123"));
        assert!(wrapped.contains("hello"));
        assert!(wrapped.contains("CCB_DONE: job_abc123"));
        assert!(wrapped.contains("End your reply with this exact final line"));
    }

    #[test]
    fn test_wrap_codex_turn_prompt() {
        let wrapped = wrap_codex_turn_prompt("hello", "job_abc123");
        assert!(wrapped.contains("CCB_REQ_ID: job_abc123"));
        assert!(wrapped.contains("hello"));
        assert!(!wrapped.contains("CCB_DONE:"));
    }
}
