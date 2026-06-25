//! Mirrors Python `lib/cli/sidebar_click.py`.

use crate::services::DaemonClient;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Parsed arguments for a sidebar mouse click.
pub struct SidebarClick {
    pub socket_path: PathBuf,
    pub mouse_y: i32,
    pub pane_top: i32,
    pub pane_height: i32,
}

/// Client interface used by [`focus_sidebar_click`].
///
/// Production implementations call the CCB daemon; tests inject fakes.
pub trait SidebarClickClient {
    /// Fetch the current project view.
    fn project_view(&self, schema_version: i64) -> Result<Value, String>;
    /// Focus the given window.
    fn project_focus_window(
        &self,
        window: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<Value, String>;
    /// Focus the given agent.
    fn project_focus_agent(
        &self,
        agent: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<Value, String>;
}

impl SidebarClickClient for crate::services::UnixDaemonClient {
    fn project_view(&self, schema_version: i64) -> Result<Value, String> {
        self.call(
            "project_view",
            serde_json::json!({"schema_version": schema_version}),
        )
    }

    fn project_focus_window(
        &self,
        window: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<Value, String> {
        let mut params = serde_json::json!({"window": window});
        if let Some(epoch) = namespace_epoch {
            params["namespace_epoch"] = serde_json::json!(epoch);
        }
        self.call("project_focus_window", params)
    }

    fn project_focus_agent(
        &self,
        agent: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<Value, String> {
        let mut params = serde_json::json!({"agent": agent});
        if let Some(epoch) = namespace_epoch {
            params["namespace_epoch"] = serde_json::json!(epoch);
        }
        self.call("project_focus_agent", params)
    }
}

/// Build the ordered list of clickable sidebar targets from a project view.
///
/// Mirrors Python `sidebar_tree_targets`: one entry per window followed by the
/// agents assigned to that window.
pub fn sidebar_tree_targets(view: &Value) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    let empty = Vec::new();
    let windows = view
        .get("windows")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    let agents = view
        .get("agents")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    for window in windows {
        let window_name = window
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if window_name.is_empty() {
            continue;
        }
        targets.push(("window".to_string(), window_name.to_string()));

        for agent in agents {
            let agent_window = agent
                .get("window")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if agent_window != window_name {
                continue;
            }
            let agent_name = agent
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if !agent_name.is_empty() {
                targets.push(("agent".to_string(), agent_name.to_string()));
            }
        }
    }
    targets
}

/// Handle a sidebar click by resolving the row to a target and focusing it.
///
/// Returns `Ok(Some("kind:name"))` when a target is focused, `Ok(None)` when
/// the click misses targets or the view is unavailable, and `Err(...)` for
/// daemon/client failures.
pub fn focus_sidebar_click<C, F>(
    click: &SidebarClick,
    client_factory: F,
) -> Result<Option<String>, String>
where
    F: FnOnce(&Path) -> C,
    C: SidebarClickClient,
{
    let relative_y = relative_coordinate(click.mouse_y, click.pane_top, click.pane_height);
    if relative_y <= 0 || relative_y >= (click.pane_height - 1).max(1) {
        return Ok(None);
    }
    let row_index = (relative_y - 1) as usize;

    let client = client_factory(&click.socket_path);
    let payload = client.project_view(1)?;
    let Some(view) = extract_view(&payload) else {
        return Ok(None);
    };

    let targets = sidebar_tree_targets(view);
    if row_index >= targets.len() {
        return Ok(None);
    }
    let (kind, name) = &targets[row_index];

    let namespace_epoch = view
        .get("namespace")
        .and_then(|v| v.as_object())
        .and_then(|ns| ns.get("epoch"))
        .and_then(|v| v.as_i64());

    match kind.as_str() {
        "window" => {
            client.project_focus_window(name, namespace_epoch)?;
        }
        _ => {
            client.project_focus_agent(name, namespace_epoch)?;
        }
    };

    Ok(Some(format!("{}:{}", kind, name)))
}

/// Parse and possibly handle a `__sidebar-click` internal command.
///
/// Returns `Ok(Some(0))` when the command was handled, `Ok(None)` when the
/// first token is not `__sidebar-click`, and `Err(...)` for parse or execution
/// failures.
pub fn maybe_handle_sidebar_click_command(args: &[String]) -> Result<Option<i32>, String> {
    if args.is_empty() || args[0] != "__sidebar-click" {
        return Ok(None);
    }
    let click = parse_sidebar_click(&args[1..])?;
    focus_sidebar_click(&click, |socket_path| {
        crate::services::UnixDaemonClient::new(socket_path.to_string_lossy().to_string())
    })?;
    Ok(Some(0))
}

fn parse_sidebar_click(args: &[String]) -> Result<SidebarClick, String> {
    let mut socket: Option<String> = None;
    let mut mouse_y: Option<i32> = None;
    let mut pane_top: Option<i32> = None;
    let mut pane_height: Option<i32> = None;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (key, value) = if let Some(eq) = arg.find('=') {
            (arg[..eq].to_string(), arg[eq + 1..].to_string())
        } else {
            let key = arg.to_string();
            let value = iter
                .next()
                .ok_or_else(|| format!("missing value for {}", key))?
                .to_string();
            (key, value)
        };

        match key.as_str() {
            "--socket" => socket = Some(value),
            "--mouse-y" => {
                mouse_y = Some(value.parse().map_err(|e| format!("bad --mouse-y: {}", e))?);
            }
            "--pane-top" => {
                pane_top = Some(
                    value
                        .parse()
                        .map_err(|e| format!("bad --pane-top: {}", e))?,
                );
            }
            "--pane-height" => {
                pane_height = Some(
                    value
                        .parse()
                        .map_err(|e| format!("bad --pane-height: {}", e))?,
                );
            }
            _ => return Err(format!("unknown argument: {}", key)),
        }
    }

    Ok(SidebarClick {
        socket_path: PathBuf::from(socket.ok_or("missing --socket")?),
        mouse_y: mouse_y.ok_or("missing --mouse-y")?,
        pane_top: pane_top.ok_or("missing --pane-top")?,
        pane_height: pane_height.ok_or("missing --pane-height")?,
    })
}

fn relative_coordinate(value: i32, pane_start: i32, pane_size: i32) -> i32 {
    // tmux normally exposes pane-relative mouse coordinates for pane bindings.
    // Keep an absolute-coordinate fallback for older or unusual format contexts.
    if value >= pane_size && value >= pane_start {
        value - pane_start
    } else {
        value
    }
}

/// Extract the nested view object from a `project_view` response.
///
/// Python `CcbdClient.project_view` returns `{"view": ..., "cache": ...}`.
/// The Rust daemon currently returns the view object directly, so fall back to
/// the payload itself when no `view` key is present.
fn extract_view(payload: &Value) -> Option<&Value> {
    if let Some(view) = payload.get("view") {
        if view.is_object() {
            return Some(view);
        }
    }
    if payload.is_object() {
        Some(payload)
    } else {
        None
    }
}
