//! Mirrors Python `lib/cli/services/tmux_project_cleanup_runtime/listing.py`.

use ccbr_terminal::backend::TmuxBackend;
use ccbr_terminal::panes::TmuxPaneService;
use ccbr_terminal::tmux::looks_like_pane_id;
use std::collections::HashMap;

/// List tmux panes owned by the project on the requested socket.
///
/// Mirrors Python `list_project_tmux_panes`.
pub fn list_project_tmux_panes(
    project_id: &str,
    socket_name: Option<&str>,
    backend: &TmuxBackend,
) -> Vec<String> {
    let project_text = project_id.trim();
    if project_text.is_empty() {
        return Vec::new();
    }

    let backend = if let Some(name) = socket_name {
        let (resolved_name, resolved_path) = super::backend::resolve_socket_ref(Some(name));
        TmuxBackend::new(resolved_name, resolved_path)
    } else {
        backend.clone()
    };

    let service = TmuxPaneService::new(backend);
    let mut expected = HashMap::new();
    expected.insert("@ccb_project_id".into(), project_text.into());
    service
        .list_panes_by_user_options(&expected)
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| looks_like_pane_id(s))
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut acc, pane| {
            if !acc.contains(&pane) {
                acc.push(pane);
            }
            acc
        })
}
