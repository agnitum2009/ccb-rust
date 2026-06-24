use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::instance_resolution::named_agent_instance;
use crate::tmux_ownership::inspect_tmux_pane_ownership;

/// A concrete provider session value.
///
/// Mirrors Python's dynamic session objects. Callers populate the fields they
/// have; `data` supplies fallback values for field extractors.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub runtime_dir: Option<PathBuf>,
    pub session_file: Option<PathBuf>,
    pub ccbr_session_id: Option<String>,
    pub data: HashMap<String, Value>,
    pub backend: Option<Arc<dyn SessionBackend>>,
    pub user_option_lookup: Option<HashMap<String, String>>,
    pub slot_user_option_lookup: Option<HashMap<String, String>>,
}

/// Backend capable of inspecting/manipulating a session.
pub trait SessionBackend: std::fmt::Debug + Send + Sync {
    fn socket_name(&self) -> Option<String>;
    fn socket_path(&self) -> Option<String>;
    fn is_alive(&self, pane_id: &str) -> bool;
    fn is_tmux_pane_alive(&self, pane_id: &str) -> bool;
    fn pane_exists(&self, pane_id: &str) -> bool;
    fn describe_pane(
        &self,
        pane_id: &str,
        user_options: &[String],
    ) -> Option<HashMap<String, String>>;
    fn list_panes_by_user_options(&self, options: &HashMap<String, String>) -> Option<Vec<String>>;
    fn set_pane_title(&self, pane_id: &str, title: &str) -> Result<(), String>;
    fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) -> Result<(), String>;
}

/// Runtime identity of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRuntimeIdentity {
    pub state: String,
    pub reason: Option<String>,
}

/// Binding information resolved for an agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentBinding {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub provider: Option<String>,
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<u32>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub ccbr_session_id: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
    pub tmux_window_id: Option<String>,
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub provider_identity_state: Option<String>,
    pub provider_identity_reason: Option<String>,
}

/// Details returned by inspecting a session's pane.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneDetails {
    pub terminal: String,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
}

/// Adapter used to load and interrogate a provider session.
///
/// Mirrors the callable fields on Python's provider session binding objects.
/// The default adapter resolver in this crate returns `None` because full
/// provider-specific loaders live in downstream crates.
pub trait BindingAdapter: std::fmt::Debug {
    /// Attribute name holding the provider session ID.
    fn session_id_attr(&self) -> &str;
    /// Attribute name holding the provider session path.
    fn session_path_attr(&self) -> &str;
    /// Load a session from `root` for the optional `instance`.
    fn load_session(&self, root: &Path, instance: Option<&str>) -> Option<Session>;
    /// Probe the live runtime identity for a loaded session.
    fn live_runtime_identity(&self, _session: &Session) -> Option<ProviderRuntimeIdentity> {
        None
    }
}

/// Default adapter resolver.
///
/// Returns `None` for every provider in this crate; downstream crates that own
/// provider-specific session loaders should supply their own resolver.
pub fn default_binding_adapter(_provider: &str) -> Option<Box<dyn BindingAdapter>> {
    None
}

/// Classify a binding given the presence of its references.
pub fn binding_status(
    runtime_ref: Option<&str>,
    session_ref: Option<&str>,
    workspace_path: Option<&str>,
) -> &'static str {
    if runtime_ref.is_some() && session_ref.is_some() && workspace_path.is_some() {
        "bound"
    } else if runtime_ref.is_some() || session_ref.is_some() || workspace_path.is_some() {
        "partial"
    } else {
        "unbound"
    }
}

/// Resolve a runtime reference from a session.
pub fn session_runtime_ref(session: &Session, pane_id_override: Option<&str>) -> Option<String> {
    let pane_id = pane_id_override
        .map(|s| s.trim())
        .or_else(|| session.pane_id.as_deref().map(|s| s.trim()))
        .filter(|s| !s.is_empty())?;
    let terminal = session
        .terminal
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("tmux");
    Some(format!("{}:{}", terminal, pane_id))
}

/// Resolve a session reference from a session.
pub fn session_ref(
    session: &Session,
    session_id_attr: &str,
    session_path_attr: &str,
) -> Option<String> {
    if let Some(token) = session.data.get(session_id_attr).and_then(Value::as_str) {
        let token = token.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    if let Some(path) = session.data.get(session_path_attr).and_then(Value::as_str) {
        let path = path.trim();
        if !path.is_empty() {
            return Some(expand_home(path));
        }
    }
    session_file(session)
}

/// Extract the tmux socket name from a session.
pub fn session_tmux_socket_name(session: &Session) -> Option<String> {
    if !session_uses_tmux(session) {
        return None;
    }
    session_data_text(session, "tmux_socket_name")
        .or_else(|| session.backend.as_ref().and_then(|b| b.socket_name()))
}

/// Extract the tmux socket path from a session.
pub fn session_tmux_socket_path(session: &Session) -> Option<String> {
    if !session_uses_tmux(session) {
        return None;
    }
    session_data_text(session, "tmux_socket_path")
        .map(|s| expand_home(&s))
        .or_else(|| session.backend.as_ref().and_then(|b| b.socket_path()))
}

/// Extract the provider session id from a session.
pub fn session_id(session: &Session, session_id_attr: &str) -> Option<String> {
    session
        .data
        .get(session_id_attr)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the CCBR session id from a session.
pub fn session_ccbr_session_id(session: &Session) -> Option<String> {
    session
        .ccbr_session_id
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| session_data_text(session, "ccbr_session_id"))
}

/// Extract the bound session file path from a session.
pub fn session_file(session: &Session) -> Option<String> {
    session
        .session_file
        .as_ref()
        .map(|p| expand_home(p.to_string_lossy().as_ref()))
}

/// Extract the runtime root directory from a session.
pub fn session_runtime_root(session: &Session) -> Option<String> {
    session
        .runtime_dir
        .as_ref()
        .map(|p| expand_home(p.to_string_lossy().as_ref()))
        .or_else(|| session_data_text(session, "runtime_dir").map(|s| expand_home(&s)))
}

/// Extract the runtime PID from a session.
pub fn session_runtime_pid(session: &Session, provider: &str) -> Option<u32> {
    if let Some(pid) = session_data_pid(session) {
        return Some(pid);
    }
    let runtime_root = session_runtime_root(session)?;
    let runtime_root = PathBuf::from(runtime_root);
    for candidate in pid_file_candidates(&runtime_root, provider) {
        if let Some(pid) = read_pid_file(&candidate) {
            return Some(pid);
        }
    }
    None
}

/// Extract the terminal name from a session.
pub fn session_terminal(session: &Session) -> Option<String> {
    session
        .terminal
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the pane title marker from a session.
pub fn session_pane_title_marker(session: &Session) -> Option<String> {
    session
        .pane_title_marker
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| session_data_text(session, "pane_title_marker"))
}

fn session_uses_tmux(session: &Session) -> bool {
    session
        .terminal
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "tmux".to_string())
        == "tmux"
}

fn session_data_text(session: &Session, key: &str) -> Option<String> {
    session
        .data
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn session_data_pid(session: &Session) -> Option<u32> {
    for key in ["runtime_pid", "pid"] {
        if let Some(value) = session.data.get(key) {
            if let Some(pid) = coerce_pid(value) {
                return Some(pid);
            }
        }
    }
    None
}

fn coerce_pid(value: &Value) -> Option<u32> {
    let text = match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>().ok().filter(|&p| p > 0)
}

fn pid_file_candidates(runtime_root: &Path, provider: &str) -> Vec<PathBuf> {
    let provider_name = provider.trim().to_lowercase();
    let preferred = runtime_root.join(format!("{}.pid", provider_name));
    let mut candidates = vec![preferred];
    if let Ok(entries) = std::fs::read_dir(runtime_root) {
        let mut extras: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("pid"))
            .collect();
        extras.sort();
        candidates.extend(extras);
    }
    candidates
}

fn read_pid_file(path: &Path) -> Option<u32> {
    if !path.is_file() {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => coerce_pid(&Value::String(text)),
        Err(_) => None,
    }
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}

fn resolve_pane_state(
    session: &Session,
    backend: &dyn SessionBackend,
    terminal: &str,
    pane_id: Option<&str>,
) -> Option<String> {
    let pane_id = pane_id?;
    if terminal == "tmux" {
        if !backend.pane_exists(pane_id) {
            return Some("missing".to_string());
        }
        let ownership = inspect_tmux_pane_ownership(session, backend, pane_id);
        if !ownership.is_owned() {
            return Some("foreign".to_string());
        }
        return Some(if backend_pane_alive(backend, pane_id) {
            "alive".to_string()
        } else {
            "dead".to_string()
        });
    }
    Some(if backend_pane_alive(backend, pane_id) {
        "alive".to_string()
    } else {
        "dead".to_string()
    })
}

fn backend_pane_alive(backend: &dyn SessionBackend, pane_id: &str) -> bool {
    if backend.is_tmux_pane_alive(pane_id) {
        return true;
    }
    backend.is_alive(pane_id)
}

/// Decide whether an existing binding must be replaced (and its pane killed).
///
/// Mirrors the stale-binding policy in Python `runtime_launch._ensure_agent_runtime`.
/// A binding is replaced when its pane is dead or its provider identity no longer
/// matches. Foreign panes are never killed here; they are left to the caller.
pub fn binding_requires_replacement(binding: &AgentBinding) -> bool {
    match binding.pane_state.as_deref() {
        Some("dead") => true,
        Some("foreign") => false,
        _ => binding.provider_identity_state.as_deref() == Some("mismatch"),
    }
}

/// Check whether a resolved agent binding's runtime is alive.
///
/// Mirrors Python `runtime_launch._binding_runtime_alive`. Rejects title-based
/// runtime references (`tmux:title:...`) and prefers the active pane id over
/// the bound pane id when probing liveness.
pub fn binding_runtime_alive(binding: &AgentBinding, backend: &dyn SessionBackend) -> bool {
    if binding
        .runtime_ref
        .as_deref()
        .unwrap_or("")
        .starts_with("tmux:title:")
    {
        return false;
    }
    let pane_id = binding
        .active_pane_id
        .as_deref()
        .or(binding.pane_id.as_deref())
        .unwrap_or("")
        .trim();
    if pane_id.is_empty() {
        return false;
    }
    backend.is_tmux_pane_alive(pane_id) || backend.is_alive(pane_id)
}

/// Inspect the pane state described by a session.
pub fn inspect_session_pane(session: &Session) -> PaneDetails {
    let terminal = session_terminal(session).unwrap_or_else(|| "tmux".to_string());
    let pane_id = session
        .pane_id
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let pane_title_marker = session_pane_title_marker(session);

    if let Some(backend) = session.backend.as_deref() {
        let pane_state = resolve_pane_state(session, backend, &terminal, pane_id.as_deref());
        let active_pane_id = if pane_state.as_deref() == Some("alive") {
            pane_id.clone()
        } else {
            None
        };
        PaneDetails {
            terminal,
            pane_id,
            active_pane_id,
            pane_title_marker,
            pane_state,
        }
    } else {
        let pane_state = if pane_id.is_some() {
            Some("unknown".to_string())
        } else if pane_title_marker.is_some() {
            Some("missing".to_string())
        } else {
            None
        };
        PaneDetails {
            terminal,
            pane_id: pane_id.clone(),
            active_pane_id: pane_id,
            pane_title_marker,
            pane_state,
        }
    }
}

fn binding_search_roots(workspace_path: &Path, project_root: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for candidate in [project_root, Some(workspace_path)].into_iter().flatten() {
        let resolved = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.to_path_buf());
        if !roots.contains(&resolved) {
            roots.push(resolved);
        }
    }
    roots
}

fn candidate_instances(provider: &str, agent_name: &str) -> Vec<Option<String>> {
    let normalized_provider = provider.trim().to_lowercase();
    let normalized_agent = agent_name.trim().to_lowercase();
    let mut candidates: Vec<Option<String>> = Vec::new();
    if let Some(instance) = named_agent_instance(agent_name, provider) {
        candidates.push(Some(instance));
    }
    if !normalized_agent.is_empty()
        && normalized_agent == normalized_provider
        && !candidates.contains(&None)
    {
        candidates.push(None);
    }
    if candidates.is_empty() {
        candidates.push(None);
    }
    candidates
}

fn should_validate_session(session: &Session) -> bool {
    if session
        .pane_id
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return true;
    }
    if session
        .pane_title_marker
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return true;
    }
    let data = &session.data;
    if data.get("active").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    let keys = [
        "pane_id",
        "tmux_session",
        "pane_title_marker",
        "runtime_dir",
        "start_cmd",
        "codex_start_cmd",
    ];
    keys.iter().any(|key| {
        data.get(*key)
            .and_then(Value::as_str)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_some()
    })
}

fn session_binding_is_usable(session: &Session, sleep_fn: &dyn Fn(Duration)) -> bool {
    if !should_validate_session(session) {
        return true;
    }
    let pane_id = session
        .pane_id
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if pane_id.is_none() {
        return true;
    }
    let pane_id = pane_id.unwrap();
    if !binding_is_stable(session, pane_id, sleep_fn) {
        return false;
    }
    binding_has_owned_tmux_pane(session, pane_id)
}

fn binding_is_stable(session: &Session, pane_id: &str, sleep_fn: &dyn Fn(Duration)) -> bool {
    let Some(backend) = session.backend.as_deref() else {
        return true;
    };
    if pane_id.is_empty() {
        return false;
    }
    if !backend.is_alive(pane_id) {
        return false;
    }
    sleep_fn(Duration::from_millis(100));
    backend.is_alive(pane_id)
}

fn binding_has_owned_tmux_pane(session: &Session, pane_id: &str) -> bool {
    let terminal = session_terminal(session).unwrap_or_else(|| "tmux".to_string());
    if terminal.to_lowercase() != "tmux" {
        return true;
    }
    let Some(backend) = session.backend.as_deref() else {
        return true;
    };
    inspect_tmux_pane_ownership(session, backend, pane_id).is_owned()
}

fn load_provider_session(
    adapter: &dyn BindingAdapter,
    provider: &str,
    agent_name: &str,
    roots: &[PathBuf],
    ensure_usable: bool,
    sleep_fn: &dyn Fn(Duration),
) -> Option<Session> {
    for root in roots {
        for instance in candidate_instances(provider, agent_name) {
            if let Some(session) = adapter.load_session(root, instance.as_deref()) {
                if !ensure_usable || session_binding_is_usable(&session, sleep_fn) {
                    return Some(session);
                }
            }
        }
    }
    None
}

/// Resolve a provider/agent binding by loading the session and inspecting it.
///
/// The `adapter_resolver` is expected to return a [`BindingAdapter`] for the
/// requested provider. The default resolver returns `None` because this crate
/// does not own provider-specific session loaders.
pub fn resolve_agent_binding(
    provider: &str,
    agent_name: &str,
    workspace_path: &Path,
    project_root: Option<&Path>,
    ensure_usable: bool,
    adapter_resolver: impl Fn(&str) -> Option<Box<dyn BindingAdapter>>,
    sleep_fn: impl Fn(Duration),
) -> Option<AgentBinding> {
    let normalized_provider = provider.trim().to_lowercase();
    if normalized_provider.is_empty() {
        return None;
    }
    let adapter = adapter_resolver(&normalized_provider)?;
    let roots = binding_search_roots(workspace_path, project_root);
    let session = load_provider_session(
        adapter.as_ref(),
        &normalized_provider,
        agent_name,
        &roots,
        ensure_usable,
        &sleep_fn,
    )?;

    let mut pane_details = inspect_session_pane(&session);
    if ensure_usable
        && pane_details.pane_state.as_deref() == Some("unknown")
        && pane_details.active_pane_id.is_some()
    {
        pane_details.pane_state = Some("alive".to_string());
    }

    let identity = adapter.live_runtime_identity(&session);

    Some(AgentBinding {
        runtime_ref: session_runtime_ref(&session, None),
        session_ref: session_ref(
            &session,
            adapter.session_id_attr(),
            adapter.session_path_attr(),
        ),
        provider: Some(normalized_provider),
        runtime_root: session_runtime_root(&session),
        runtime_pid: session_runtime_pid(&session, provider),
        session_file: session_file(&session),
        session_id: session_id(&session, adapter.session_id_attr()),
        ccbr_session_id: session_ccbr_session_id(&session),
        tmux_socket_name: session_tmux_socket_name(&session),
        tmux_socket_path: session_tmux_socket_path(&session),
        terminal: session_terminal(&session),
        pane_id: pane_details.pane_id,
        active_pane_id: pane_details.active_pane_id,
        pane_title_marker: pane_details.pane_title_marker,
        pane_state: pane_details.pane_state,
        provider_identity_state: identity.as_ref().map(|i| i.state.clone()),
        provider_identity_reason: identity.as_ref().and_then(|i| i.reason.clone()),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestBackend {
        socket_name: String,
        socket_path: String,
    }

    impl SessionBackend for TestBackend {
        fn socket_name(&self) -> Option<String> {
            Some(self.socket_name.clone())
        }
        fn socket_path(&self) -> Option<String> {
            Some(self.socket_path.clone())
        }
        fn is_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn is_tmux_pane_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn pane_exists(&self, _pane_id: &str) -> bool {
            true
        }
        fn describe_pane(
            &self,
            _pane_id: &str,
            _user_options: &[String],
        ) -> Option<HashMap<String, String>> {
            None
        }
        fn list_panes_by_user_options(
            &self,
            _options: &HashMap<String, String>,
        ) -> Option<Vec<String>> {
            None
        }
        fn set_pane_title(&self, _pane_id: &str, _title: &str) -> Result<(), String> {
            Ok(())
        }
        fn set_pane_user_option(
            &self,
            _pane_id: &str,
            _name: &str,
            _value: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_binding_status() {
        assert_eq!(binding_status(Some("r"), Some("s"), Some("w")), "bound");
        assert_eq!(binding_status(Some("r"), None, None), "partial");
        assert_eq!(binding_status(None, Some("s"), None), "partial");
        assert_eq!(binding_status(None, None, Some("w")), "partial");
        assert_eq!(binding_status(Some("r"), Some("s"), None), "partial");
        assert_eq!(binding_status(None, None, None), "unbound");
    }

    #[test]
    fn test_session_ref_falls_back_from_id_to_path_to_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_file = tmp.path().join("session.jsonl");
        std::fs::write(&session_file, "{}").unwrap();

        let mut session = Session {
            session_file: Some(session_file.clone()),
            ..Default::default()
        };
        assert_eq!(
            session_ref(&session, "session_id", "session_path"),
            Some(session_file.to_string_lossy().to_string())
        );

        session.data.insert(
            "session_path".to_string(),
            Value::String("/tmp/explicit.jsonl".to_string()),
        );
        assert_eq!(
            session_ref(&session, "session_id", "session_path"),
            Some("/tmp/explicit.jsonl".to_string())
        );

        session.data.insert(
            "session_id".to_string(),
            Value::String("sess-abc".to_string()),
        );
        assert_eq!(
            session_ref(&session, "session_id", "session_path"),
            Some("sess-abc".to_string())
        );
    }

    #[test]
    fn test_session_tmux_socket_fields_prefer_session_data() {
        let tmp = std::env::temp_dir();
        let mut session = Session {
            terminal: Some("tmux".to_string()),
            ..Default::default()
        };
        session.data.insert(
            "tmux_socket_name".to_string(),
            Value::String("proj-sock".to_string()),
        );
        session.data.insert(
            "tmux_socket_path".to_string(),
            Value::String(tmp.join("tmux.sock").to_string_lossy().to_string()),
        );
        session.backend = Some(Arc::new(TestBackend {
            socket_name: "backend-sock".to_string(),
            socket_path: "/tmp/backend.sock".to_string(),
        }));

        assert_eq!(
            session_tmux_socket_name(&session),
            Some("proj-sock".to_string())
        );
        assert_eq!(
            session_tmux_socket_path(&session),
            Some(tmp.join("tmux.sock").to_string_lossy().to_string())
        );
    }

    #[test]
    fn test_session_ccbr_session_id_prefers_attribute_then_data() {
        let mut session = Session {
            ccbr_session_id: Some("direct-session".to_string()),
            ..Default::default()
        };
        assert_eq!(
            session_ccbr_session_id(&session),
            Some("direct-session".to_string())
        );

        session.ccbr_session_id = None;
        session.data.insert(
            "ccbr_session_id".to_string(),
            Value::String("payload-session".to_string()),
        );
        assert_eq!(
            session_ccbr_session_id(&session),
            Some("payload-session".to_string())
        );
    }

    #[test]
    fn test_session_runtime_pid_prefers_data_then_provider_pid_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let runtime_dir = tmp.path().join("runtime");
        std::fs::create_dir(&runtime_dir).unwrap();
        std::fs::write(runtime_dir.join("codex.pid"), "123\n").unwrap();
        std::fs::write(runtime_dir.join("other.pid"), "456\n").unwrap();

        let session = Session {
            runtime_dir: Some(runtime_dir.clone()),
            ..Default::default()
        };
        assert_eq!(session_runtime_pid(&session, "codex"), Some(123));

        let mut session_with_data = Session {
            runtime_dir: Some(runtime_dir),
            ..Default::default()
        };
        session_with_data
            .data
            .insert("runtime_pid".to_string(), Value::String("789".to_string()));
        assert_eq!(session_runtime_pid(&session_with_data, "codex"), Some(789));
    }

    #[test]
    fn test_inspect_session_pane_without_backend() {
        let session = Session {
            terminal: Some("tmux".to_string()),
            pane_id: Some("%1".to_string()),
            ..Default::default()
        };
        let details = inspect_session_pane(&session);
        assert_eq!(details.terminal, "tmux");
        assert_eq!(details.pane_id, Some("%1".to_string()));
        assert_eq!(details.pane_state, Some("unknown".to_string()));
    }

    #[test]
    fn test_inspect_session_pane_missing_with_marker() {
        let session = Session {
            terminal: Some("tmux".to_string()),
            pane_title_marker: Some("claude".to_string()),
            ..Default::default()
        };
        let details = inspect_session_pane(&session);
        assert_eq!(details.pane_state, Some("missing".to_string()));
    }

    #[derive(Debug)]
    struct AliveBackend;

    impl SessionBackend for AliveBackend {
        fn socket_name(&self) -> Option<String> {
            None
        }
        fn socket_path(&self) -> Option<String> {
            None
        }
        fn is_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn is_tmux_pane_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn pane_exists(&self, _pane_id: &str) -> bool {
            true
        }
        fn describe_pane(
            &self,
            _pane_id: &str,
            _user_options: &[String],
        ) -> Option<HashMap<String, String>> {
            None
        }
        fn list_panes_by_user_options(
            &self,
            _options: &HashMap<String, String>,
        ) -> Option<Vec<String>> {
            None
        }
        fn set_pane_title(&self, _pane_id: &str, _title: &str) -> Result<(), String> {
            Ok(())
        }
        fn set_pane_user_option(
            &self,
            _pane_id: &str,
            _name: &str,
            _value: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_inspect_session_pane_alive() {
        let session = Session {
            terminal: Some("tmux".to_string()),
            pane_id: Some("%1".to_string()),
            backend: Some(Arc::new(AliveBackend)),
            ..Default::default()
        };
        let details = inspect_session_pane(&session);
        assert_eq!(details.pane_state, Some("alive".to_string()));
        assert_eq!(details.active_pane_id, Some("%1".to_string()));
    }

    #[derive(Debug)]
    struct TestAdapter {
        session_id_attr: String,
        session_path_attr: String,
    }

    impl BindingAdapter for TestAdapter {
        fn session_id_attr(&self) -> &str {
            &self.session_id_attr
        }
        fn session_path_attr(&self) -> &str {
            &self.session_path_attr
        }
        fn load_session(&self, _root: &Path, instance: Option<&str>) -> Option<Session> {
            let mut session = Session {
                pane_id: Some("%1".to_string()),
                terminal: Some("tmux".to_string()),
                ..Default::default()
            };
            session.data.insert(
                "session_id".to_string(),
                Value::String("sess-123".to_string()),
            );
            session
                .data
                .insert("runtime_pid".to_string(), Value::String("42".to_string()));
            if let Some(inst) = instance {
                session
                    .data
                    .insert("instance".to_string(), Value::String(inst.to_string()));
            }
            Some(session)
        }
        fn live_runtime_identity(&self, _session: &Session) -> Option<ProviderRuntimeIdentity> {
            Some(ProviderRuntimeIdentity {
                state: "ready".to_string(),
                reason: Some("probe-ok".to_string()),
            })
        }
    }

    #[test]
    fn test_resolve_agent_binding_happy_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let binding = resolve_agent_binding(
            "claude",
            "claude",
            tmp.path(),
            None,
            false,
            |_provider| {
                Some(Box::new(TestAdapter {
                    session_id_attr: "session_id".to_string(),
                    session_path_attr: "session_path".to_string(),
                }) as Box<dyn BindingAdapter>)
            },
            |_duration| {},
        )
        .expect("binding should resolve");

        assert_eq!(binding.provider, Some("claude".to_string()));
        assert_eq!(binding.session_id, Some("sess-123".to_string()));
        assert_eq!(binding.runtime_pid, Some(42));
        assert_eq!(binding.provider_identity_state, Some("ready".to_string()));
        assert_eq!(binding.pane_state, Some("unknown".to_string()));
    }

    #[test]
    fn test_resolve_agent_binding_no_adapter_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(resolve_agent_binding(
            "claude",
            "claude",
            tmp.path(),
            None,
            false,
            |_provider| None,
            |_duration| {},
        )
        .is_none());
    }

    #[test]
    fn test_default_binding_adapter_returns_none() {
        assert!(default_binding_adapter("claude").is_none());
    }

    #[derive(Debug)]
    struct RecordingBackend {
        alive_panes: Vec<String>,
        tmux_alive_panes: Vec<String>,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl SessionBackend for RecordingBackend {
        fn socket_name(&self) -> Option<String> {
            None
        }
        fn socket_path(&self) -> Option<String> {
            None
        }
        fn is_alive(&self, pane_id: &str) -> bool {
            self.calls
                .lock()
                .unwrap()
                .push(format!("is_alive:{pane_id}"));
            self.alive_panes.contains(&pane_id.to_string())
        }
        fn is_tmux_pane_alive(&self, pane_id: &str) -> bool {
            self.calls
                .lock()
                .unwrap()
                .push(format!("is_tmux_pane_alive:{pane_id}"));
            self.tmux_alive_panes.contains(&pane_id.to_string())
        }
        fn pane_exists(&self, _pane_id: &str) -> bool {
            true
        }
        fn describe_pane(
            &self,
            _pane_id: &str,
            _user_options: &[String],
        ) -> Option<HashMap<String, String>> {
            None
        }
        fn list_panes_by_user_options(
            &self,
            _options: &HashMap<String, String>,
        ) -> Option<Vec<String>> {
            None
        }
        fn set_pane_title(&self, _pane_id: &str, _title: &str) -> Result<(), String> {
            Ok(())
        }
        fn set_pane_user_option(
            &self,
            _pane_id: &str,
            _name: &str,
            _value: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_binding_runtime_alive_prefers_active_pane() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let backend = RecordingBackend {
            alive_panes: vec![],
            tmux_alive_panes: vec!["%77".to_string()],
            calls: calls.clone(),
        };
        let binding = AgentBinding {
            runtime_ref: Some("tmux:%41".to_string()),
            active_pane_id: Some("%77".to_string()),
            pane_id: Some("%41".to_string()),
            ..Default::default()
        };
        assert!(binding_runtime_alive(&binding, &backend));
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "is_tmux_pane_alive:%77");
    }

    #[test]
    fn test_binding_runtime_alive_rejects_title_based_runtime_ref() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let backend = RecordingBackend {
            alive_panes: vec![],
            tmux_alive_panes: vec!["%41".to_string()],
            calls: calls.clone(),
        };
        let binding = AgentBinding {
            runtime_ref: Some("tmux:title:CCBR-agent1-demo".to_string()),
            pane_title_marker: Some("CCBR-agent1-demo".to_string()),
            pane_id: Some("%41".to_string()),
            ..Default::default()
        };
        assert!(!binding_runtime_alive(&binding, &backend));
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_binding_runtime_alive_false_when_pane_dead() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let backend = RecordingBackend {
            alive_panes: vec![],
            tmux_alive_panes: vec![],
            calls: calls.clone(),
        };
        let binding = AgentBinding {
            runtime_ref: Some("tmux:%41".to_string()),
            pane_id: Some("%41".to_string()),
            ..Default::default()
        };
        assert!(!binding_runtime_alive(&binding, &backend));
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn test_binding_requires_replacement_policy() {
        assert!(binding_requires_replacement(&AgentBinding {
            pane_state: Some("dead".to_string()),
            ..Default::default()
        }));
        assert!(!binding_requires_replacement(&AgentBinding {
            pane_state: Some("foreign".to_string()),
            provider_identity_state: Some("mismatch".to_string()),
            ..Default::default()
        }));
        assert!(binding_requires_replacement(&AgentBinding {
            pane_state: Some("alive".to_string()),
            provider_identity_state: Some("mismatch".to_string()),
            ..Default::default()
        }));
        assert!(!binding_requires_replacement(&AgentBinding {
            pane_state: Some("alive".to_string()),
            provider_identity_state: Some("match".to_string()),
            ..Default::default()
        }));
        assert!(binding_requires_replacement(&AgentBinding {
            provider_identity_state: Some("mismatch".to_string()),
            ..Default::default()
        }));
    }
}
