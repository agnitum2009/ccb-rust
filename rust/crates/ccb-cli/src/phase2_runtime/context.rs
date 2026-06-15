//! Mirrors Python `lib/cli/phase2_runtime/context.py`.
//! 1:1 file alignment.

use std::path::Path;

use crate::context::CliContext;
use crate::context::CliContextBuilder;

/// Determine if a command should bootstrap the project if missing.
///
/// Mirrors Python `should_bootstrap_if_missing(command) -> bool`.
///
/// # Arguments
/// * `kind` - Command kind string
///
/// # Returns
/// true if project should be bootstrapped when missing, false otherwise
pub fn should_bootstrap_if_missing(kind: &str) -> bool {
    !matches!(kind, "cleanup" | "config-validate" | "kill" | "reload")
}

/// Build phase2 execution context with optional reset handling.
///
/// Mirrors Python `build_context(command, *, cwd, out, builder_cls, ...)` - simplified
/// Rust version that delegates to CliContextBuilder.
///
/// This function handles the special case of `start --reset-context`, which requires
/// confirming with the user before rebuilding the project context.
///
/// # Arguments
/// * `command` - Parsed command value with `kind` and optional `reset_context` fields
/// * `cwd` - Current working directory (defaults to current dir if None)
/// * `builder` - CliContextBuilder instance
///
/// # Returns
/// Built CliContext
///
/// # Errors
/// Returns CliContextError if context building fails
///
/// # Note
/// Reset confirmation logic is simplified in this Rust implementation. The full Python
/// version includes interactive confirmation and project state reset which will be wired
/// when the daemon runtime is available.
pub fn build_context(
    command: &serde_json::Value,
    cwd: Option<&Path>,
    builder: &CliContextBuilder,
) -> Result<CliContext, crate::context::CliContextError> {
    let kind = command.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let reset_context = command.get("reset_context").and_then(|v| v.as_bool()).unwrap_or(false);

    // Handle reset start context
    if kind == "start" && reset_context {
        // NOTE: Full reset flow with confirmation and state reset will be wired
        // when daemon runtime is available. For now, build fresh context.
        return builder.build();
    }

    // Standard context build
    builder.build()
}

/// Resolve existing context without bootstrapping if project is missing.
///
/// Mirrors Python `resolve_existing_context(command, *, cwd, builder_cls, ...)`
/// - simplified for current Rust implementation.
///
/// This function attempts to build context but returns Ok(None) instead of an error
/// if the project cannot be discovered (i.e., project_discovery_error_cls is raised).
///
/// # Arguments
/// * `command` - Parsed command value
/// * `cwd` - Current working directory
/// * `builder` - CliContextBuilder instance
///
/// # Returns
/// Ok(Some(CliContext)) if context built successfully, Ok(None) if project not found
///
/// # Note
/// In the full Python version, this catches project_discovery_error_cls. The Rust
/// implementation will be extended when project discovery error types are fully
/// wired into the CliContextBuilder.
pub fn resolve_existing_context(
    command: &serde_json::Value,
    cwd: &Path,
    builder: &CliContextBuilder,
) -> Result<Option<CliContext>, crate::context::CliContextError> {
    match builder.build() {
        Ok(ctx) => Ok(Some(ctx)),
        Err(e) => {
            // Check if error is due to missing project
            // For now, we'll return None for any build error
            // This will be refined when CliContextError discriminates project discovery failures
            let error_msg = e.to_string();
            if error_msg.contains("NoProjectRoot") || error_msg.contains("project root") {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

/// Resolve requested project root from command or cwd.
///
/// Mirrors Python `resolve_requested_project_root(command, *, cwd, ...)`
/// - simplified version using Path operations.
///
/// # Arguments
/// * `command` - Parsed command value with optional `project` field
/// * `cwd` - Current working directory
///
/// # Returns
/// Resolved project root path
///
/// # Errors
/// Returns error if project root does not exist or is not a directory
///
/// # Note
/// This is a placeholder implementation. The full Python version raises
/// project_discovery_error_cls with detailed messaging. The Rust version
/// will be extended with proper error types when project discovery is fully implemented.
pub fn resolve_requested_project_root(
    command: &serde_json::Value,
    cwd: &Path,
) -> Result<std::path::PathBuf, String> {
    let project = command.get("project").and_then(|v| v.as_str());

    let root = if let Some(proj) = project {
        // Expand user home if present (~)
        if proj.starts_with('~') {
            // TODO: Implement proper home directory expansion
            // For now, treat as relative path
            std::path::PathBuf::from(proj)
        } else {
            std::path::PathBuf::from(proj)
        }
    } else {
        cwd.to_path_buf()
    };

    // Try to resolve to absolute path
    let resolved = if root.exists() {
        root.canonicalize().unwrap_or_else(|_| root.clone().to_path_buf())
    } else {
        root.clone().to_path_buf()
    };

    // Validate that path exists and is a directory
    if !resolved.exists() || !resolved.is_dir() {
        return Err(format!("project root not found: {}", resolved.display()));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_bootstrap_if_missing() {
        // Commands that should NOT bootstrap
        assert!(!should_bootstrap_if_missing("cleanup"));
        assert!(!should_bootstrap_if_missing("config-validate"));
        assert!(!should_bootstrap_if_missing("kill"));
        assert!(!should_bootstrap_if_missing("reload"));

        // Commands that SHOULD bootstrap
        assert!(should_bootstrap_if_missing("start"));
        assert!(should_bootstrap_if_missing("ask"));
        assert!(should_bootstrap_if_missing("ping"));
        assert!(should_bootstrap_if_missing("doctor"));
    }

    #[test]
    fn test_resolve_requested_project_root() {
        let cwd = Path::new("/tmp");
        let command = serde_json::json!({});

        // Test with no project field - should return cwd
        // Note: This test will fail if /tmp doesn't exist
        if cwd.exists() {
            let result = resolve_requested_project_root(&command, cwd);
            assert!(result.is_ok());
        }
    }
}
