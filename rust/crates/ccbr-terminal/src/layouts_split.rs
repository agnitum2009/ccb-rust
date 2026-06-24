//! Mirrors Python lib/terminal_runtime/layouts_split.py
// TODO: translate from Python

/// Split pane direction
pub fn normalize_split_direction(direction: &str) -> String {
    match direction.to_lowercase().as_str() {
        "h" | "horizontal" => "h".to_string(),
        "v" | "vertical" => "v".to_string(),
        _ => direction.to_string(),
    }
}

/// Split pane with direction
pub fn split_pane(
    backend: &dyn crate::layouts::TmuxLayoutBackend,
    parent_pane_id: &str,
    direction: &str,
    percent: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    // TODO: implement pane splitting
    Ok(String::new())
}
