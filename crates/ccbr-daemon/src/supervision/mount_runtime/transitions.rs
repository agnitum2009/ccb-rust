//! Mirrors Python `lib/ccbrd/supervision/mount_runtime/transitions.py`.
//!
//! Pure helpers that decide what to do before/after a mount attempt and how to
//! persist the resulting runtime state.

use ccbr_agents::models::AgentRuntime;

use super::events::{record_mount_failed, record_mount_superseded, MountEventStore};

/// Health values that indicate a successful mount.
pub const SUCCESS_RUNTIME_HEALTHS: &[&str] = &["healthy", "restored"];

/// Per-agent mount action callback.
type MountAgentFn<'a> = &'a dyn Fn(&str) -> Result<(), MountActionError>;

/// Project-wide remount action callback.
type RemountProjectFn<'a> = &'a dyn Fn(&str) -> Result<(), MountActionError>;

/// Factory that builds a starting runtime for a new mount attempt.
type BuildStartingRuntimeFn<'a> =
    &'a dyn Fn(&str, Option<&AgentRuntime>, &str) -> crate::Result<AgentRuntime>;

/// Returns `true` when neither per-agent nor project-wide mount actions are available.
pub fn mount_actions_missing(
    mount_agent_fn: Option<MountAgentFn<'_>>,
    remount_project_fn: Option<RemountProjectFn<'_>>,
) -> bool {
    mount_agent_fn.is_none() && remount_project_fn.is_none()
}

/// Health to report when no mount action is configured.
pub fn missing_mount_action_health(runtime: Option<&AgentRuntime>) -> String {
    runtime
        .map(|r| r.health.clone())
        .unwrap_or_else(|| "unmounted".to_string())
}

/// Returns `true` when the runtime is still inside its backoff window.
pub fn in_backoff_window(
    runtime: Option<&AgentRuntime>,
    attempted_at: &str,
    is_in_backoff_window_fn: &dyn Fn(&AgentRuntime, &str) -> bool,
) -> bool {
    runtime.is_some_and(|r| is_in_backoff_window_fn(r, attempted_at))
}

/// Build the "starting" runtime for a new mount attempt.
///
/// Returns `(starting_runtime, prior_health, next_restart_count)`.
pub fn start_mount_attempt(
    agent_name: &str,
    runtime: Option<&AgentRuntime>,
    attempted_at: &str,
    build_starting_runtime_fn: BuildStartingRuntimeFn<'_>,
) -> crate::Result<(AgentRuntime, String, u32)> {
    let starting = build_starting_runtime_fn(agent_name, runtime, attempted_at)?;
    let prior_health = runtime
        .map(|r| r.health.clone())
        .unwrap_or_else(|| "unmounted".to_string());
    let next_restart_count = starting.restart_count.saturating_add(1);
    Ok((starting, prior_health, next_restart_count))
}

/// Persist an exception-style mount failure and emit a `mount_failed` event.
pub fn persist_mount_exception(
    finalized: &AgentRuntime,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    event_store: &dyn MountEventStore,
    reason: &str,
) -> String {
    record_mount_failed(
        event_store,
        project_id,
        agent_name,
        attempted_at,
        prior_health,
        finalized,
        reason,
    );
    finalized.health.clone()
}

/// Persist a transient mount failure and emit a `mount_failed` event.
pub fn persist_mount_transient(
    finalized: &AgentRuntime,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    reason: &str,
    event_store: &dyn MountEventStore,
) -> String {
    record_mount_failed(
        event_store,
        project_id,
        agent_name,
        attempted_at,
        prior_health,
        finalized,
        reason,
    );
    finalized.health.clone()
}

/// No-op persistence for a successful mount (mirrors Python identity helper).
pub fn persist_mount_success(finalized: AgentRuntime) -> AgentRuntime {
    finalized
}

/// Persist a superseded mount attempt and emit a `mount_superseded` event.
pub fn persist_mount_superseded(
    current: &AgentRuntime,
    project_id: &str,
    agent_name: &str,
    attempted_at: &str,
    prior_health: &str,
    event_store: &dyn MountEventStore,
    attempt_id: &str,
) -> String {
    record_mount_superseded(
        event_store,
        project_id,
        agent_name,
        attempted_at,
        prior_health,
        current,
        attempt_id,
    );
    current.health.clone()
}

/// Decide whether to remount the whole project namespace or just this agent.
pub fn mount_or_reflow(
    agent_name: &str,
    mount_agent_fn: Option<MountAgentFn<'_>>,
    remount_project_fn: Option<RemountProjectFn<'_>>,
    should_reflow_project_mount_fn: &dyn Fn(&str) -> bool,
) -> Result<(), MountActionError> {
    if should_reflow_project_mount_fn(agent_name) {
        if let Some(remount) = remount_project_fn {
            return remount(&format!("mount_recovery:{agent_name}"));
        }
        // Fall through to per-agent mount if no project remount is configured.
    }
    if let Some(mount) = mount_agent_fn {
        return mount(agent_name);
    }
    Err(MountActionError::Fatal(format!(
        "no mount action available for {agent_name}"
    )))
}

/// Error returned by a mount action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountActionError {
    /// The tmux server was temporarily unavailable; the attempt should be deferred.
    Transient(String),
    /// A non-recoverable mount failure.
    Fatal(String),
}

impl std::fmt::Display for MountActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountActionError::Transient(msg) | MountActionError::Fatal(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for MountActionError {}

impl From<ccbr_terminal::readiness::TmuxTransientServerUnavailable> for MountActionError {
    fn from(err: ccbr_terminal::readiness::TmuxTransientServerUnavailable) -> Self {
        MountActionError::Transient(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_actions_missing_both_none() {
        assert!(mount_actions_missing(None, None));
    }

    #[test]
    fn test_mount_actions_missing_one_present() {
        let mount = |_name: &str| Ok::<(), MountActionError>(());
        assert!(!mount_actions_missing(Some(&mount), None));
        assert!(!mount_actions_missing(None, Some(&mount)));
    }

    #[test]
    fn test_missing_mount_action_health() {
        assert_eq!(missing_mount_action_health(None), "unmounted");
        let runtime = AgentRuntime {
            health: "starting".to_string(),
            ..AgentRuntime::default()
        };
        assert_eq!(missing_mount_action_health(Some(&runtime)), "starting");
    }

    #[test]
    fn test_in_backoff_window() {
        let runtime = AgentRuntime {
            health: "failed".to_string(),
            ..AgentRuntime::default()
        };
        let backoff = |_: &AgentRuntime, _: &str| true;
        let no_backoff = |_: &AgentRuntime, _: &str| false;
        assert!(in_backoff_window(Some(&runtime), "now", &backoff));
        assert!(!in_backoff_window(Some(&runtime), "now", &no_backoff));
        assert!(!in_backoff_window(None, "now", &backoff));
    }

    #[test]
    fn test_mount_or_reflow_prefers_reflow() {
        let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let calls2 = calls.clone();
        let mount = move |name: &str| {
            calls2.borrow_mut().push(format!("mount:{name}"));
            Ok(())
        };
        let calls3 = calls.clone();
        let remount = move |key: &str| {
            calls3.borrow_mut().push(format!("remount:{key}"));
            Ok(())
        };
        let should_reflow = |_name: &str| true;
        mount_or_reflow("claude", Some(&mount), Some(&remount), &should_reflow).unwrap();
        assert_eq!(*calls.borrow(), vec!["remount:mount_recovery:claude"]);
    }

    #[test]
    fn test_mount_or_reflow_falls_back_to_mount() {
        let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let calls2 = calls.clone();
        let mount = move |name: &str| {
            calls2.borrow_mut().push(format!("mount:{name}"));
            Ok(())
        };
        let should_reflow = |_name: &str| false;
        mount_or_reflow("claude", Some(&mount), None, &should_reflow).unwrap();
        assert_eq!(*calls.borrow(), vec!["mount:claude"]);
    }

    #[test]
    fn test_mount_or_reflow_no_action_is_fatal() {
        let should_reflow = |_name: &str| false;
        let err = mount_or_reflow("claude", None, None, &should_reflow).unwrap_err();
        assert!(matches!(err, MountActionError::Fatal(_)));
    }

    #[test]
    fn test_persist_mount_success_identity() {
        let runtime = AgentRuntime {
            health: "healthy".to_string(),
            ..AgentRuntime::default()
        };
        let out = persist_mount_success(runtime.clone());
        assert_eq!(out.health, "healthy");
    }
}
