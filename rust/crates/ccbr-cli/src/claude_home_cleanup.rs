//! Mirrors Python `lib/cli/management_runtime/claude_home_cleanup.py`.

use std::collections::{BTreeSet, HashSet};

/// Retired ask-shortcut commands installed as Claude command docs.
pub const RETIRED_ASK_SHORTCUT_COMMANDS: &[&str] = &[
    "bask", "cask", "dask", "gask", "hask", "lask", "oask", "qask",
];

/// Retired pend commands installed as Claude command docs.
pub const RETIRED_PEND_COMMANDS: &[&str] = &[
    "bpend", "cpend", "dpend", "gpend", "hpend", "lpend", "opend", "qpend",
];

/// Retired ping commands installed as Claude command docs.
pub const RETIRED_PING_COMMANDS: &[&str] = &[
    "bping", "cping", "dping", "gping", "hping", "lping", "oping", "qping",
];

/// Permission allow literals that should be removed from Claude settings.
pub const RETIRED_PERMISSION_ALLOW_LITERALS: &[&str] = &[
    "Bash(ccb provider ping *)",
    "Bash(ccb provider pend *)",
    "Bash(ccbr-ping *)",
    "Bash(pend *)",
];

/// Return the sorted list of retired command-doc filenames.
pub fn claude_command_docs() -> Vec<String> {
    let mut docs = BTreeSet::new();
    for command in RETIRED_ASK_SHORTCUT_COMMANDS {
        docs.insert(format!("{command}.md"));
    }
    for command in RETIRED_PEND_COMMANDS {
        docs.insert(format!("{command}.md"));
    }
    for command in RETIRED_PING_COMMANDS {
        docs.insert(format!("{command}.md"));
    }
    docs.into_iter().collect()
}

/// Return the set of retired `permissions.allow` entries that should be
/// stripped from Claude `settings.json`.
pub fn retired_permission_allow_entries() -> HashSet<String> {
    let mut entries = HashSet::new();
    for command in RETIRED_ASK_SHORTCUT_COMMANDS {
        entries.insert(format!("Bash({command}:*)"));
    }
    for literal in RETIRED_PERMISSION_ALLOW_LITERALS {
        entries.insert((*literal).to_string());
    }
    for command in RETIRED_PEND_COMMANDS {
        entries.insert(format!("Bash({command})"));
    }
    for command in RETIRED_PING_COMMANDS {
        entries.insert(format!("Bash({command})"));
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_command_docs_cover_wrapper_registries() {
        let docs = claude_command_docs();
        assert!(docs.contains(&"cask.md".to_string()));
        assert!(docs.contains(&"gpend.md".to_string()));
        assert!(docs.contains(&"qpend.md".to_string()));
        assert!(docs.contains(&"cping.md".to_string()));
        assert!(docs.contains(&"hping.md".to_string()));
    }

    #[test]
    fn test_cleanup_permissions_cover_wrapper_registries() {
        let entries = retired_permission_allow_entries();
        assert!(entries.contains("Bash(cask:*)"));
        assert!(entries.contains("Bash(cpend)"));
        assert!(entries.contains("Bash(qpend)"));
        assert!(entries.contains("Bash(cping)"));
        assert!(entries.contains("Bash(hping)"));
    }
}
