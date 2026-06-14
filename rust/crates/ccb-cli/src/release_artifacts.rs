/// Normalize architecture string to standard form.
///
/// Maps common architecture aliases (amd64→x86_64, arm64→aarch64).
pub fn normalize_arch(raw_arch: &str) -> String {
    let text = raw_arch.trim().to_lowercase();
    let mapping = std::collections::HashMap::from([
        ("x86_64", "x86_64"),
        ("amd64", "x86_64"),
        ("aarch64", "aarch64"),
        ("arm64", "aarch64"),
    ]);
    mapping
        .get(text.as_str())
        .copied()
        .unwrap_or({
            if text.is_empty() {
                "unknown"
            } else {
                text.as_str()
            }
        })
        .to_string()
}

/// Normalize release platform string.
///
/// Maps platform names (Linux→linux, Darwin→macos).
/// Returns None if platform is not recognized.
pub fn normalize_release_platform(raw_system: &str) -> Option<String> {
    let text = raw_system.trim();
    let mapping = std::collections::HashMap::from([
        ("Linux", "linux"),
        ("Darwin", "macos"),
        ("linux", "linux"),
        ("macos", "macos"),
    ]);
    mapping.get(text).copied().map(|s| s.to_string())
}

/// Get the architecture for a release build on a given platform.
///
/// For Linux: returns normalized machine architecture.
/// For macOS: returns "universal" (always).
/// Returns None for unknown platforms.
pub fn release_build_arch(platform_name: &str, machine: &str) -> Option<String> {
    let platform = normalize_release_platform(platform_name)?;
    match platform.as_str() {
        "linux" => Some(normalize_arch(machine)),
        "macos" => Some("universal".to_string()),
        _ => None,
    }
}

/// Get the base name for a release artifact.
///
/// For Linux: returns "ccb-linux-{arch}"
/// For macOS: returns "ccb-macos-universal"
/// Returns None for unknown platforms.
pub fn release_artifact_basename(platform_name: &str, machine: &str) -> Option<String> {
    let platform = normalize_release_platform(platform_name)?;
    match platform.as_str() {
        "linux" => {
            let arch = normalize_arch(machine);
            if arch == "unknown" {
                None
            } else {
                Some(format!("ccb-linux-{arch}"))
            }
        }
        "macos" => Some("ccb-macos-universal".to_string()),
        _ => None,
    }
}

/// Get the full filename for a release artifact.
///
/// Returns "{basename}.tar.gz" or None if platform is unknown.
pub fn release_artifact_name(platform_name: &str, machine: &str) -> Option<String> {
    let basename = release_artifact_basename(platform_name, machine)?;
    Some(format!("{basename}.tar.gz"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_arch() {
        // Direct mappings
        assert_eq!(normalize_arch("x86_64"), "x86_64");
        assert_eq!(normalize_arch("amd64"), "x86_64");
        assert_eq!(normalize_arch("AMD64"), "x86_64");
        assert_eq!(normalize_arch("aarch64"), "aarch64");
        assert_eq!(normalize_arch("arm64"), "aarch64");
        assert_eq!(normalize_arch("ARM64"), "aarch64");

        // Empty/unknown
        assert_eq!(normalize_arch(""), "unknown");
        assert_eq!(normalize_arch("riscv64"), "riscv64");
    }

    #[test]
    fn test_normalize_release_platform() {
        // Linux variants
        assert_eq!(
            normalize_release_platform("Linux"),
            Some("linux".to_string())
        );
        assert_eq!(
            normalize_release_platform("linux"),
            Some("linux".to_string())
        );

        // macOS variants
        assert_eq!(
            normalize_release_platform("Darwin"),
            Some("macos".to_string())
        );
        assert_eq!(
            normalize_release_platform("macos"),
            Some("macos".to_string())
        );

        // Unknown platforms
        assert_eq!(normalize_release_platform("Windows"), None);
        assert_eq!(normalize_release_platform(""), None);
    }

    #[test]
    fn test_release_build_arch() {
        // Linux returns normalized arch
        assert_eq!(
            release_build_arch("Linux", "x86_64"),
            Some("x86_64".to_string())
        );
        assert_eq!(
            release_build_arch("linux", "amd64"),
            Some("x86_64".to_string())
        );
        assert_eq!(
            release_build_arch("Linux", "aarch64"),
            Some("aarch64".to_string())
        );

        // macOS returns universal
        assert_eq!(
            release_build_arch("Darwin", "x86_64"),
            Some("universal".to_string())
        );
        assert_eq!(
            release_build_arch("macos", "arm64"),
            Some("universal".to_string())
        );

        // Unknown platforms
        assert_eq!(release_build_arch("Windows", "x86_64"), None);
    }

    #[test]
    fn test_release_artifact_basename() {
        // Linux artifacts
        assert_eq!(
            release_artifact_basename("Linux", "x86_64"),
            Some("ccb-linux-x86_64".to_string())
        );
        assert_eq!(
            release_artifact_basename("linux", "amd64"),
            Some("ccb-linux-x86_64".to_string())
        );
        assert_eq!(
            release_artifact_basename("Linux", "aarch64"),
            Some("ccb-linux-aarch64".to_string())
        );

        // macOS artifact
        assert_eq!(
            release_artifact_basename("Darwin", "x86_64"),
            Some("ccb-macos-universal".to_string())
        );
        assert_eq!(
            release_artifact_basename("macos", "arm64"),
            Some("ccb-macos-universal".to_string())
        );

        // Unknown platforms
        assert_eq!(release_artifact_basename("Windows", "x86_64"), None);
    }

    #[test]
    fn test_release_artifact_name() {
        // Full artifact names
        assert_eq!(
            release_artifact_name("Linux", "x86_64"),
            Some("ccb-linux-x86_64.tar.gz".to_string())
        );
        assert_eq!(
            release_artifact_name("Darwin", "arm64"),
            Some("ccb-macos-universal.tar.gz".to_string())
        );

        // Unknown platforms
        assert_eq!(release_artifact_name("Windows", "x86_64"), None);
    }
}
