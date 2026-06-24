//! Top-level convenience API matching Python `terminal_runtime.api`.

use std::sync::Mutex;

use serde_json::Value;

use crate::backend::TmuxBackend;
use crate::layouts::{create_tmux_auto_layout, LayoutResult};

static BACKEND_CACHE: Mutex<Option<TmuxBackend>> = Mutex::new(None);

/// Create an automatic tmux layout for a list of providers.
pub fn create_auto_layout(
    providers: &[String],
    cwd: &str,
    root_pane_id: Option<&str>,
    tmux_session_name: Option<&str>,
    percent: u32,
    set_markers: bool,
    marker_prefix: &str,
) -> anyhow::Result<LayoutResult> {
    let backend = TmuxBackend::new(None, None);
    let detached_session_name = crate::tmux::default_detached_session_name(
        cwd,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    );
    let inside_tmux = std::env::var("TMUX")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    create_tmux_auto_layout(
        providers,
        cwd,
        &backend,
        root_pane_id,
        tmux_session_name,
        percent,
        set_markers,
        marker_prefix,
        Some(&detached_session_name),
        inside_tmux,
    )
}

/// Detect the current terminal type.
///
/// Returns `Some("tmux")` when running inside tmux, otherwise `None`.
pub fn detect_terminal() -> Option<String> {
    crate::detect::detect_terminal()
}

/// Resolve the tmux backend for the current terminal.
pub fn get_backend(terminal_type: Option<&str>) -> Option<TmuxBackend> {
    let mut cache = BACKEND_CACHE.lock().unwrap();
    if let Some(backend) = cache.as_ref() {
        return Some(backend.clone());
    }
    let selected = terminal_type
        .map(|s| s.to_string())
        .or_else(crate::detect::detect_terminal)?;
    if selected == "tmux" {
        let backend = TmuxBackend::new(None, None);
        *cache = Some(backend.clone());
        Some(backend)
    } else {
        None
    }
}

/// Resolve a tmux backend from session data (socket name/path optional).
pub fn get_backend_for_session(session_data: &Value) -> Option<TmuxBackend> {
    let socket_name = session_data
        .get("tmux_socket_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let socket_path = session_data
        .get("tmux_socket_path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some(TmuxBackend::new(socket_name, socket_path))
}

/// Extract a pane id from session data.
pub fn get_pane_id_from_session(session_data: &Value) -> Option<String> {
    session_data
        .get("pane_id")
        .or_else(|| session_data.get("tmux_session"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Infer the shell type for the current platform.
pub fn get_shell_type() -> String {
    if crate::env::is_windows()
        && std::env::var("CCBR_BACKEND_ENV")
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default()
            == "wsl"
    {
        return "bash".to_string();
    }
    let (shell, _) = crate::env::default_shell();
    if shell == "pwsh" || shell == "powershell" {
        "powershell".to_string()
    } else {
        "bash".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_backend_cache() {
        let _ = BACKEND_CACHE.lock().unwrap().take();
    }

    #[test]
    fn test_get_shell_type_bash_on_unix() {
        std::env::remove_var("CCBR_BACKEND_ENV");
        #[cfg(not(windows))]
        assert_eq!(get_shell_type(), "bash");
    }

    #[test]
    fn test_get_shell_type_wsl_env() {
        std::env::set_var("CCBR_BACKEND_ENV", "wsl");
        assert_eq!(get_shell_type(), "bash");
        std::env::remove_var("CCBR_BACKEND_ENV");
    }

    #[test]
    fn test_get_backend_outside_tmux() {
        reset_backend_cache();
        std::env::remove_var("TMUX");
        std::env::remove_var("TMUX_PANE");
        assert!(get_backend(None).is_none());
        assert!(get_backend(Some("windows")).is_none());
    }

    #[test]
    fn test_get_backend_explicit_tmux() {
        reset_backend_cache();
        let backend = get_backend(Some("tmux"));
        assert!(backend.is_some());
    }

    #[test]
    fn test_get_backend_caches() {
        reset_backend_cache();
        let first = get_backend(Some("tmux")).unwrap();
        let second = get_backend(Some("tmux")).unwrap();
        assert_eq!(first.socket_name(), second.socket_name());
        assert_eq!(first.socket_path(), second.socket_path());
    }

    #[test]
    fn test_get_backend_for_session() {
        let session = serde_json::json!({
            "tmux_socket_name": "ccb",
            "tmux_socket_path": "/tmp/ccb.sock",
        });
        let backend = get_backend_for_session(&session).unwrap();
        assert_eq!(backend.socket_name(), Some("ccb"));
        assert_eq!(backend.socket_path(), Some("/tmp/ccb.sock"));
    }

    #[test]
    fn test_get_pane_id_from_session() {
        let session = serde_json::json!({ "pane_id": "%1" });
        assert_eq!(get_pane_id_from_session(&session), Some("%1".to_string()));

        let session = serde_json::json!({ "tmux_session": "ccbr-demo" });
        assert_eq!(
            get_pane_id_from_session(&session),
            Some("ccbr-demo".to_string())
        );

        let session = serde_json::json!({});
        assert_eq!(get_pane_id_from_session(&session), None);
    }

    #[test]
    fn test_create_auto_layout_errors_on_empty_providers() {
        let result = create_auto_layout(&[], "/tmp", None, None, 50, true, "CCB");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_auto_layout_errors_on_too_many_providers() {
        let providers: Vec<String> = (0..5).map(|i| format!("agent{i}")).collect();
        let result = create_auto_layout(&providers, "/tmp", None, None, 50, true, "CCB");
        assert!(result.is_err());
    }
}
