use std::path::Path;

/// Normalize a path for directory matching.
pub fn normalize_path_for_match(value: &str) -> String {
    let raw = value.trim();
    if raw.is_empty() {
        return String::new();
    }
    let path = Path::new(raw);
    let resolved = if let Ok(r) = path.canonicalize() {
        r
    } else {
        path.to_path_buf()
    };
    let normalized = resolved
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    #[cfg(target_os = "windows")]
    {
        normalized = normalized.to_lowercase();
    }
    normalized
}

/// Check if `parent` is the same as or a parent of `child`.
pub fn path_is_same_or_parent(parent: &str, child: &str) -> bool {
    let normalized_parent = normalize_path_for_match(parent);
    let normalized_child = normalize_path_for_match(child);
    if normalized_parent.is_empty() || normalized_child.is_empty() {
        return false;
    }
    if normalized_parent == normalized_child {
        return true;
    }
    if !normalized_child.starts_with(&normalized_parent) {
        return false;
    }
    normalized_child[normalized_parent.len()..].starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_for_match_empty() {
        assert_eq!(normalize_path_for_match(""), "");
        assert_eq!(normalize_path_for_match("   "), "");
    }

    #[test]
    fn test_path_is_same_or_parent() {
        assert!(path_is_same_or_parent("/tmp", "/tmp/foo"));
        assert!(path_is_same_or_parent("/tmp", "/tmp"));
        assert!(!path_is_same_or_parent("/tmp", "/tmpfoo"));
        assert!(!path_is_same_or_parent("/tmp", "/foo"));
        assert!(!path_is_same_or_parent("", "/tmp"));
    }
}
