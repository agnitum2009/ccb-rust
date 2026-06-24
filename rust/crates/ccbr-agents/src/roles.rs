use serde::{Deserialize, Serialize};

pub const LEGACY_ROLE_ALIASES: &[(&str, &str)] = &[
    ("ccb.archi", "agentroles.archi"),
    ("agentrole.ccbr_self", "agentroles.ccbr_self"),
];

/// Resolve a role id through legacy aliases.
pub fn canonical_role_id(role_id: &str) -> String {
    let normalized = role_id.trim().to_lowercase();
    for (legacy, canonical) in LEGACY_ROLE_ALIASES {
        if normalized == *legacy {
            return (*canonical).into();
        }
    }
    normalized
}

/// Return the legacy ids that map to the canonical id.
pub fn legacy_role_ids(canonical_id: &str) -> Vec<String> {
    let canonical = canonical_role_id(canonical_id);
    LEGACY_ROLE_ALIASES
        .iter()
        .filter(|(_, target)| *target == canonical.as_str())
        .map(|(legacy, _)| (*legacy).into())
        .collect()
}

/// Return candidate role ids to search for, including canonical and legacy forms.
pub fn role_id_candidates(role_id: &str) -> Vec<String> {
    let canonical = canonical_role_id(role_id);
    let mut candidates = vec![canonical.clone()];
    for legacy in legacy_role_ids(&canonical) {
        if !candidates.contains(&legacy) {
            candidates.push(legacy);
        }
    }
    candidates
}

/// Resolve a role alias to its canonical form.
pub fn canonical_role_alias(role: &str) -> String {
    let role_lower = role.trim().to_lowercase();
    match role_lower.as_str() {
        "dev" => "developer".into(),
        "eng" => "engineer".into(),
        "qa" => "tester".into(),
        "review" => "reviewer".into(),
        "docs" => "documenter".into(),
        _ => role_lower,
    }
}

/// Role definition for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    pub role_id: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default)]
    pub startup_args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_role_id() {
        assert_eq!(canonical_role_id("dev"), "dev");
        assert_eq!(canonical_role_id("ccb.archi"), "agentroles.archi");
    }

    #[test]
    fn test_role_id_candidates() {
        let candidates = role_id_candidates("agentroles.archi");
        assert!(candidates.contains(&"ccb.archi".to_string()));
    }

    #[test]
    fn test_canonical_role_alias() {
        assert_eq!(canonical_role_alias("dev"), "developer");
        assert_eq!(canonical_role_alias("custom"), "custom");
    }
}
