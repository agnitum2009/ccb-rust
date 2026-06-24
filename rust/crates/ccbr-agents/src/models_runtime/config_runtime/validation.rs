//! Mirrors Python `lib/agents/models_runtime/config_runtime/validation.py`.

use crate::layout::{build_balanced_layout, parse_layout_spec};
use std::collections::HashMap;

/// Resolve a layout specification string into a normalized rendered layout.
///
/// Mirrors Python `resolve_layout_spec`. When `layout_spec` is non-empty it is
/// parsed and re-rendered so that structural normalization (e.g. parentheses,
/// percent tokens) is preserved. When empty, a balanced layout is built from
/// `default_agents` and `cmd_enabled`.
pub fn resolve_layout_spec(
    default_agents: &[String],
    _normalized_agents: &HashMap<String, crate::models::AgentSpec>,
    cmd_enabled: bool,
    layout_spec: &str,
) -> Result<String, String> {
    let spec = layout_spec.trim();
    if spec.is_empty() {
        if default_agents.is_empty() {
            return Err("no agents available for default layout".into());
        }
        let node = build_balanced_layout(default_agents, None, None, cmd_enabled);
        return Ok(node.render());
    }
    let node = parse_layout_spec(spec).map_err(|e| e.to_string())?;
    Ok(node.render())
}
