//! Mirrors Python `lib/cli/services/runtime_launch_runtime/binding_state_runtime/common.py`.

use serde_json::Value;

/// Build a tmux backend instance for the given binding.
///
/// Mirrors Python `build_tmux_backend(binding, tmux_backend_cls)`.
pub fn build_tmux_backend<F>(binding: &Value, tmux_backend_cls: F) -> Option<Value>
where
    F: Fn(Option<&str>, Option<&str>) -> Option<Value>,
{
    let socket_name = binding.get("tmux_socket_name").and_then(|v| v.as_str());
    let socket_path = binding.get("tmux_socket_path").and_then(|v| v.as_str());
    // Try with socket_name and socket_path first
    if let Some(result) = tmux_backend_cls(socket_name, socket_path) {
        return Some(result);
    }
    // Fallback: call without parameters
    tmux_backend_cls(None, None)
}

/// Extract the target pane ID from a binding's runtime reference.
///
/// Mirrors Python `tmux_target_pane_id(binding, runtime_ref)`.
pub fn tmux_target_pane_id(binding: &Value, runtime_ref: &str) -> String {
    let active_pane_id = binding.get("active_pane_id").and_then(|v| v.as_str());
    let pane_id = binding.get("pane_id").and_then(|v| v.as_str());
    let result = active_pane_id
        .or(pane_id)
        .unwrap_or_else(|| {
            runtime_ref.strip_prefix("tmux:").unwrap_or(runtime_ref)
        });
    result.trim().to_string()
}
