//! Mirrors Python `lib/cli/render_runtime/ops_views_common.py`.

use serde_json::Value;

pub fn binding_line(agent: &Value) -> String {
    let get = |key: &str| agent.get(key).and_then(|v| v.as_str()).unwrap_or("");
    format!(
        "binding: status={} runtime={} session={} source={} workspace={} terminal={} \
         socket={} socket_path={} window={} window_id={} pane={} active_pane={} \
         pane_state={} marker={}",
        get("binding_status"),
        get("runtime_ref"),
        get("session_ref"),
        agent
            .get("binding_source")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        get("workspace_path"),
        agent.get("terminal").and_then(|v| v.as_str()).unwrap_or(""),
        agent
            .get("tmux_socket_name")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        agent
            .get("tmux_socket_path")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        agent
            .get("tmux_window_name")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        agent
            .get("tmux_window_id")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        agent.get("pane_id").and_then(|v| v.as_str()).unwrap_or(""),
        agent
            .get("active_pane_id")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        agent
            .get("pane_state")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        agent
            .get("pane_title_marker")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    )
}
