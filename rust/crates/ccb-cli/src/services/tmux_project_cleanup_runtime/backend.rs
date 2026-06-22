//! Mirrors Python `lib/cli/services/tmux_project_cleanup_runtime/backend.py`.

use ccb_terminal::backend::TmuxBackend;

fn expanduser(path: &str) -> String {
    let trimmed = path.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        return format!("{}/{}", home.trim_end_matches('/'), rest);
    }
    if trimmed == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    }
    trimmed.into()
}

/// Resolve a socket reference into the name/path pair used by `TmuxBackend`.
///
/// Mirrors Python `resolve_socket_ref`.
pub fn resolve_socket_ref(socket_name: Option<&str>) -> (Option<String>, Option<String>) {
    let text = socket_name.unwrap_or("").trim();
    if text.is_empty() {
        return (None, None);
    }
    if text.contains('/') || text.contains('\\') {
        return (None, Some(expanduser(text)));
    }
    (Some(text.into()), None)
}

/// Build a `TmuxBackend` for the requested socket.
///
/// Mirrors Python `build_backend`. The Rust `TmuxBackend::new` constructor is
/// already tolerant of missing/ambiguous socket arguments, so no variant loop
/// is required for the default factory.
pub fn build_backend(socket_name: Option<&str>) -> Option<TmuxBackend> {
    let (name, path) = resolve_socket_ref(socket_name);
    Some(TmuxBackend::new(name, path))
}

/// Check whether the `tmux` binary is available.
///
/// Tests may set `CCB_TEST_TMUX_AVAILABLE=1` to bypass the real binary check.
pub fn tmux_available() -> bool {
    if std::env::var("CCB_TEST_TMUX_AVAILABLE")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return true;
    }
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_socket_ref_empty() {
        assert_eq!(resolve_socket_ref(None), (None, None));
        assert_eq!(resolve_socket_ref(Some("  ")), (None, None));
    }

    #[test]
    fn test_resolve_socket_ref_name() {
        assert_eq!(resolve_socket_ref(Some("ccb")), (Some("ccb".into()), None));
    }

    #[test]
    fn test_resolve_socket_ref_path() {
        let (name, path) = resolve_socket_ref(Some("/tmp/ccb.sock"));
        assert_eq!(name, None);
        assert_eq!(path, Some("/tmp/ccb.sock".into()));
    }

    #[test]
    fn test_resolve_socket_ref_expands_tilde() {
        std::env::set_var("HOME", "/home/tester");
        let (name, path) = resolve_socket_ref(Some("~/tmux.sock"));
        assert_eq!(name, None);
        assert_eq!(path, Some("/home/tester/tmux.sock".into()));
    }
}
