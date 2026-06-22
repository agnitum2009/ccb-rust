//! Mirrors Python `lib/cli/services/tmux_project_cleanup_runtime/killing.py`.

use ccb_terminal::backend::TmuxBackend;
use ccb_terminal::tmux::looks_like_pane_id;

/// Kill the given tmux panes, deferring the current pane until last.
///
/// Mirrors Python `kill_panes`.
pub fn kill_panes(
    pane_ids: &[String],
    socket_name: Option<&str>,
    backend: &TmuxBackend,
    current_pane_id: Option<&str>,
) -> Vec<String> {
    let backend = if let Some(name) = socket_name {
        let (resolved_name, resolved_path) = super::backend::resolve_socket_ref(Some(name));
        TmuxBackend::new(resolved_name, resolved_path)
    } else {
        backend.clone()
    };

    let current = current_pane_id.unwrap_or("").trim().to_string();
    let mut ordered: Vec<String> = pane_ids
        .iter()
        .filter(|p| **p != current)
        .cloned()
        .collect();
    if looks_like_pane_id(&current) && pane_ids.contains(&current) {
        ordered.push(current);
    }

    let mut killed = Vec::new();
    for pane_id in ordered {
        if backend.kill_tmux_pane(&pane_id).is_ok() {
            killed.push(pane_id);
        }
    }
    killed
}
