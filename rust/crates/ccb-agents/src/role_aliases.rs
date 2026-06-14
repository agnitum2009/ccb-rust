/// Legacy role ID alias mappings.
static LEGACY_ROLE_ALIASES: &[(&str, &str)] = &[
    ("ccb.archi", "agentroles.archi"),
    ("agentrole.ccb_self", "agentroles.ccb_self"),
];

fn legacy_aliases_map() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for (legacy, canonical) in LEGACY_ROLE_ALIASES {
        map.insert(legacy.to_string(), canonical.to_string());
    }
    map
}

/// Normalize a role ID to its canonical form.
///
/// If the role ID is a legacy alias, returns the canonical ID.
/// Otherwise returns the input normalized (lowercased, stripped).
pub fn canonical_role_id(role_id: &str) -> String {
    let normalized = role_id.trim().to_lowercase();
    let aliases = legacy_aliases_map();
    aliases.get(&normalized).cloned().unwrap_or(normalized)
}

/// Get all legacy role IDs that map to the given canonical ID.
///
/// Returns a sorted tuple of legacy role IDs.
pub fn legacy_role_ids(canonical_id: &str) -> Vec<String> {
    let canonical = canonical_role_id(canonical_id);
    let aliases = legacy_aliases_map();
    let mut legacy_ids: Vec<String> = aliases
        .iter()
        .filter(|(_, target)| **target == canonical)
        .map(|(legacy, _)| legacy.clone())
        .collect();
    legacy_ids.sort();
    legacy_ids
}

/// Get all candidate role IDs for a given role ID.
///
/// Returns candidates starting with the canonical ID, followed by any legacy IDs.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_role_id() {
        // Legacy aliases map to canonical
        assert_eq!(canonical_role_id("ccb.archi"), "agentroles.archi");
        assert_eq!(
            canonical_role_id("agentrole.ccb_self"),
            "agentroles.ccb_self"
        );

        // Non-legacy IDs are normalized (lowercased)
        assert_eq!(canonical_role_id("AgentRoles.Test"), "agentroles.test");
        assert_eq!(canonical_role_id("  agentroles.test  "), "agentroles.test");

        // Unknown IDs pass through normalized
        assert_eq!(canonical_role_id("unknown.role"), "unknown.role");
    }

    #[test]
    fn test_legacy_role_ids() {
        // Canonical IDs with legacy mappings
        let legacy = legacy_role_ids("agentroles.archi");
        assert_eq!(legacy, vec!["ccb.archi"]);

        let legacy = legacy_role_ids("agentroles.ccb_self");
        assert_eq!(legacy, vec!["agentrole.ccb_self"]);

        // IDs with no legacy mappings return empty
        let legacy = legacy_role_ids("agentroles.unknown");
        assert!(legacy.is_empty());
    }

    #[test]
    fn test_role_id_candidates() {
        // Legacy alias returns canonical first
        let candidates = role_id_candidates("ccb.archi");
        assert_eq!(candidates, vec!["agentroles.archi", "ccb.archi"]);

        // Canonical ID returns itself first, then legacy
        let candidates = role_id_candidates("agentroles.archi");
        assert_eq!(candidates, vec!["agentroles.archi", "ccb.archi"]);

        // Unknown ID returns just itself normalized
        let candidates = role_id_candidates("unknown.role");
        assert_eq!(candidates, vec!["unknown.role"]);
    }
}
