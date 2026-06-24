use ccbr_provider_core::protocol::REQ_ID_PREFIX;

/// Wrap a Mimo prompt with the request anchor prefix.
pub fn wrap_mimo_prompt(message: &str, req_id: &str) -> String {
    let message = message.trim_end();
    format!("{} {}\n\n{}\n", REQ_ID_PREFIX, req_id, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_mimo_prompt_format() {
        let wrapped = wrap_mimo_prompt("do the thing", "req-12345678");
        assert!(wrapped.contains("req-12345678"));
        assert!(wrapped.contains("do the thing"));
        assert!(wrapped.starts_with(REQ_ID_PREFIX));
    }
}
