//! Mirrors Python `lib/ccbd/services/health_assessment/tmux_runtime/namespace.py`.

/// Runtime information needed to decide pane namespace membership.
#[derive(Debug, Clone)]
pub struct RuntimeInfo {
    pub project_id: String,
    pub agent_name: String,
    pub slot_key: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
}

/// Loaded project namespace state.
#[derive(Debug, Clone)]
pub struct NamespaceStateInfo {
    pub tmux_socket_path: Option<String>,
    pub tmux_session_name: String,
    pub workspace_window_id: Option<String>,
}

/// A record returned by inspecting a tmux pane in the project namespace.
pub trait PaneRecord {
    fn window_id(&self) -> Option<&str>;
    fn window_name(&self) -> Option<&str>;
    fn ccb_window(&self) -> Option<&str>;
    fn matches(
        &self,
        tmux_session_name: &str,
        project_id: &str,
        role: &str,
        slot_key: Option<&str>,
        window_name: Option<&str>,
        managed_by: &str,
    ) -> bool;
}

/// Store that can load the current project namespace state.
pub trait NamespaceStateStore {
    fn load(&self) -> Option<NamespaceStateInfo>;
}

/// Backend that can check socket membership and inspect a pane's namespace
/// record.
pub trait TmuxNamespaceBackend {
    fn backend_socket_matches(&self, tmux_socket_path: Option<&str>) -> bool;
    fn inspect_project_namespace_pane(&self, pane_id: &str) -> Option<Box<dyn PaneRecord>>;
}

/// Returns `true` if the pane is outside the project's namespace.
///
/// Mirrors Python `pane_outside_project_namespace`.
pub fn pane_outside_project_namespace<S, B>(
    runtime: &RuntimeInfo,
    namespace_state_store: &S,
    backend: Option<&B>,
    pane_id: &str,
) -> bool
where
    S: NamespaceStateStore,
    B: TmuxNamespaceBackend,
{
    let pane_text = normalized_tmux_pane_id(pane_id);
    let pane_text = match pane_text {
        Some(p) => p,
        None => return false,
    };
    let backend = match backend {
        Some(b) => b,
        None => return false,
    };
    let namespace_state = match namespace_state_store.load() {
        Some(s) => s,
        None => return false,
    };
    if !backend.backend_socket_matches(namespace_state.tmux_socket_path.as_deref()) {
        return runtime_socket_matches_namespace(
            runtime,
            namespace_state.tmux_socket_path.as_deref(),
        );
    }
    let record = backend.inspect_project_namespace_pane(&pane_text);
    record_outside_namespace(runtime, &namespace_state, record.as_deref())
}

fn normalized_tmux_pane_id(pane_id: &str) -> Option<String> {
    let pane_text = pane_id.trim();
    if pane_text.starts_with('%') {
        Some(pane_text.to_string())
    } else {
        None
    }
}

fn runtime_socket_matches_namespace(runtime: &RuntimeInfo, tmux_socket_path: Option<&str>) -> bool {
    let runtime_socket = runtime.tmux_socket_path.as_deref().unwrap_or("").trim();
    if runtime_socket.is_empty() {
        return false;
    }
    Some(runtime_socket) == tmux_socket_path
}

fn record_outside_namespace(
    runtime: &RuntimeInfo,
    namespace_state: &NamespaceStateInfo,
    record: Option<&dyn PaneRecord>,
) -> bool {
    let record = match record {
        Some(r) => r,
        None => return true,
    };
    let slot_key = runtime
        .slot_key
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(Some(runtime.agent_name.as_str()));
    let window_name = runtime
        .tmux_window_name
        .as_deref()
        .filter(|s| !s.is_empty());
    let matches = record.matches(
        &namespace_state.tmux_session_name,
        &runtime.project_id,
        "agent",
        slot_key,
        window_name,
        "ccbd",
    );
    if !matches {
        return true;
    }
    if record_matches_runtime_window(runtime, record) {
        return false;
    }
    let workspace_window_id = namespace_state
        .workspace_window_id
        .as_deref()
        .filter(|s| !s.is_empty());
    let record_window_id = record.window_id().filter(|s| !s.is_empty());
    match (workspace_window_id, record_window_id) {
        (Some(wid), Some(rid)) => rid != wid,
        _ => false,
    }
}

fn record_matches_runtime_window(runtime: &RuntimeInfo, record: &dyn PaneRecord) -> bool {
    let window_name = match runtime
        .tmux_window_name
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        Some(w) => w,
        None => return false,
    };
    if record.ccb_window().filter(|s| !s.is_empty()) == Some(window_name) {
        return true;
    }
    if record.window_name().filter(|s| !s.is_empty()) == Some(window_name) {
        return true;
    }
    false
}
