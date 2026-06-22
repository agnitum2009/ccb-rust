//! Mirrors Python `lib/cli/services/tmux_project_cleanup_runtime/cleanup.py`.

use ccb_terminal::backend::TmuxBackend;
use std::collections::HashMap;

use super::backend::tmux_available;
use super::killing::kill_panes;
use super::listing::list_project_tmux_panes;
use super::models::ProjectTmuxCleanupSummary;

/// List project tmux panes on a socket.
///
/// Mirrors Python `list_project_tmux_panes` top-level export.
pub fn list_project_tmux_panes_owned(
    project_id: &str,
    socket_name: Option<&str>,
    backend: &TmuxBackend,
) -> Vec<String> {
    if !tmux_available() {
        return Vec::new();
    }
    list_project_tmux_panes(project_id, socket_name, backend)
}

/// Kill panes that are owned by the project but not in the active set.
///
/// Mirrors Python `cleanup_project_tmux_orphans`.
pub fn cleanup_project_tmux_orphans(
    project_id: &str,
    active_panes: &[String],
    socket_name: Option<&str>,
    backend: &TmuxBackend,
    current_pane_id: Option<&str>,
) -> Vec<String> {
    if !tmux_available() {
        return Vec::new();
    }
    let active: std::collections::HashSet<String> = active_panes
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| ccb_terminal::tmux::looks_like_pane_id(s))
        .collect();
    let owned = list_project_tmux_panes(project_id, socket_name, backend);
    if owned.is_empty() {
        return Vec::new();
    }
    let orphaned: Vec<String> = owned.into_iter().filter(|p| !active.contains(p)).collect();
    if orphaned.is_empty() {
        return Vec::new();
    }
    kill_panes(&orphaned, socket_name, backend, current_pane_id)
}

/// Clean up orphans across multiple sockets, returning per-socket summaries.
///
/// Mirrors Python `cleanup_project_tmux_orphans_by_socket`.
pub fn cleanup_project_tmux_orphans_by_socket(
    project_id: &str,
    active_panes_by_socket: HashMap<Option<String>, Vec<String>>,
    backend: &TmuxBackend,
    current_pane_id: Option<&str>,
) -> Vec<ProjectTmuxCleanupSummary> {
    if !tmux_available() {
        return Vec::new();
    }
    let mut socket_names: Vec<Option<String>> = active_panes_by_socket.keys().cloned().collect();
    if !active_panes_by_socket.contains_key(&None) {
        socket_names.push(None);
    }

    let mut summaries = Vec::new();
    for socket_name in socket_names {
        let owned = list_project_tmux_panes(project_id, socket_name.as_deref(), backend);
        if owned.is_empty() {
            continue;
        }
        let active: Vec<String> = active_panes_by_socket
            .get(&socket_name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| ccb_terminal::tmux::looks_like_pane_id(s))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let orphaned: Vec<String> = owned
            .iter()
            .filter(|p| !active.contains(*p))
            .cloned()
            .collect();
        let killed = if orphaned.is_empty() {
            Vec::new()
        } else {
            kill_panes(&orphaned, socket_name.as_deref(), backend, current_pane_id)
        };
        summaries.push(ProjectTmuxCleanupSummary {
            socket_name: socket_name.clone(),
            owned_panes: owned.clone(),
            active_panes: active.clone(),
            orphaned_panes: orphaned,
            killed_panes: killed,
        });
    }
    summaries
}

/// Kill all project tmux panes on a socket.
///
/// Mirrors Python `kill_project_tmux_panes`.
pub fn kill_project_tmux_panes(
    project_id: &str,
    socket_name: Option<&str>,
    backend: &TmuxBackend,
    current_pane_id: Option<&str>,
) -> Vec<String> {
    if !tmux_available() {
        return Vec::new();
    }
    let panes = list_project_tmux_panes(project_id, socket_name, backend);
    if panes.is_empty() {
        return Vec::new();
    }
    kill_panes(&panes, socket_name, backend, current_pane_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccb_terminal::backend::{TmuxBackend, TmuxOutput};

    fn success_output(stdout: &str) -> TmuxOutput {
        TmuxOutput {
            stdout: stdout.into(),
            stderr: String::new(),
            status: std::process::Command::new("true").status().unwrap(),
        }
    }

    fn mock_backend_with_panes(panes: &[(&str, &str)]) -> TmuxBackend {
        let lines: Vec<String> = panes
            .iter()
            .map(|(id, project_id)| format!("{id}\t{project_id}"))
            .collect();
        let stdout = lines.join("\n");
        TmuxBackend::new(None, None).with_runner(
            move |args: Vec<String>, _check: bool, _capture: bool, _input, _timeout, _env| {
                if args.iter().any(|a| a == "list-panes") {
                    return Ok(success_output(&stdout));
                }
                if args.iter().any(|a| a == "kill-pane") {
                    return Ok(success_output(""));
                }
                Ok(success_output(""))
            },
        )
    }

    struct EnvGuard {
        had_var: bool,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set() -> Self {
            let previous = std::env::var("CCB_TEST_TMUX_AVAILABLE").ok();
            std::env::set_var("CCB_TEST_TMUX_AVAILABLE", "1");
            Self {
                had_var: previous.is_some(),
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if self.had_var {
                if let Some(ref v) = self.previous {
                    std::env::set_var("CCB_TEST_TMUX_AVAILABLE", v);
                }
            } else {
                std::env::remove_var("CCB_TEST_TMUX_AVAILABLE");
            }
        }
    }

    #[test]
    fn test_list_project_tmux_panes_owned_filters_by_project_id() {
        let _guard = EnvGuard::set();
        let backend = mock_backend_with_panes(&[("%1", "proj-a"), ("%2", "proj-b")]);
        let panes = list_project_tmux_panes_owned("proj-a", None, &backend);
        assert_eq!(panes, vec!["%1"]);
    }

    #[test]
    fn test_cleanup_project_tmux_orphans_kills_non_active() {
        let _guard = EnvGuard::set();
        let backend = mock_backend_with_panes(&[("%1", "proj"), ("%2", "proj")]);
        let killed = cleanup_project_tmux_orphans("proj", &["%1".into()], None, &backend, None);
        assert_eq!(killed, vec!["%2"]);
    }

    #[test]
    fn test_cleanup_project_tmux_orphans_by_socket_returns_summary() {
        let _guard = EnvGuard::set();
        let backend = mock_backend_with_panes(&[("%1", "proj"), ("%2", "proj")]);
        let mut active = HashMap::new();
        active.insert(None, vec!["%1".into()]);
        let summaries = cleanup_project_tmux_orphans_by_socket("proj", active, &backend, None);
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.socket_name, None);
        assert_eq!(summary.owned_panes, vec!["%1", "%2"]);
        assert_eq!(summary.active_panes, vec!["%1"]);
        assert_eq!(summary.orphaned_panes, vec!["%2"]);
        assert_eq!(summary.killed_panes, vec!["%2"]);
    }

    #[test]
    fn test_kill_project_tmux_panes_kills_all_owned() {
        let _guard = EnvGuard::set();
        let backend = mock_backend_with_panes(&[("%1", "proj"), ("%2", "proj")]);
        let killed = kill_project_tmux_panes("proj", None, &backend, None);
        assert_eq!(killed, vec!["%1", "%2"]);
    }
}
