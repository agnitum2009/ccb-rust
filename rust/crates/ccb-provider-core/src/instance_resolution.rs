/// Normalize an agent name into an instance identifier.
///
/// The `primary_agent` argument is accepted for API compatibility with the
/// Python implementation but is ignored.
pub fn named_agent_instance(agent_name: &str, _primary_agent: &str) -> Option<String> {
    let normalized = agent_name.trim().to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// Determine whether session resolution should fall back to the primary
/// agent session.
pub fn should_fallback_to_primary_session(agent_name: &str, _primary_agent: &str) -> bool {
    named_agent_instance(agent_name, "").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_agent_instance() {
        assert_eq!(
            named_agent_instance("  Reviewer  ", "claude"),
            Some("reviewer".to_string())
        );
        assert_eq!(named_agent_instance("", "claude"), None);
    }

    #[test]
    fn test_should_fallback_to_primary_session() {
        assert!(should_fallback_to_primary_session("", "claude"));
        assert!(!should_fallback_to_primary_session("reviewer", "claude"));
    }
}
