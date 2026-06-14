//! Mirrors Python lib/terminal_runtime/layouts_root.py
// TODO: translate from Python

/// Resolve root pane
pub fn resolve_root_pane(
    backend: &dyn crate::layouts::TmuxLayoutBackend,
    cwd: &str,
    root_pane_id: Option<String>,
    tmux_session_name: Option<String>,
    detached_session_name: Option<String>,
    inside_tmux: bool,
) -> Result<(String, bool, Vec<String>), Box<dyn std::error::Error>> {
    // TODO: implement root pane resolution
    Ok((String::new(), false, Vec::new()))
}

/// Create detached root pane
pub fn detached_root_pane(
    backend: &dyn crate::layouts::TmuxLayoutBackend,
    cwd: &str,
    session_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // TODO: implement detached root pane creation
    Ok(String::new())
}

/// Get first pane ID from output
pub fn first_pane_id(stdout: &str) -> String {
    stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .next()
        .unwrap_or("")
        .to_string()
}
