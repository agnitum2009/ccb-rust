//! Mirrors Python `lib/provider_execution/active_runtime/start.py`.

use std::path::{Path, PathBuf};

use ccbr_completion::models::{CompletionSourceKind, JobRecord};
use serde_json::Value;

use crate::execution::{error_submission, ProviderRuntimeContext, ProviderSubmission};

use super::models::PreparedActiveStart;
use super::resume::ActiveSession;

/// Outcome of preparing an active-mode provider start.
/// Mirrors the Python union `ProviderSubmission | PreparedActiveStart`.
#[derive(Debug, Clone)]
pub enum StartOutcome<S> {
    Error(ProviderSubmission),
    Prepared(PreparedActiveStart<S>),
}

impl<S> StartOutcome<S> {
    /// Return the prepared start or panic if this is an error submission.
    #[cfg(test)]
    pub fn expect_prepared(self) -> PreparedActiveStart<S> {
        match self {
            StartOutcome::Prepared(p) => p,
            StartOutcome::Error(_) => panic!("expected a prepared start, got an error submission"),
        }
    }
}

/// Determine which agent/session name should be used when loading a session.
///
/// Mirrors Python `_session_selector_name`:
/// prefer `job.provider_instance`, then `job.agent_name`, then `job.provider`.
pub fn session_selector_name(job: &JobRecord) -> String {
    if let Some(instance) = job.provider_instance.as_deref() {
        let instance = instance.trim();
        if !instance.is_empty() {
            return instance.to_string();
        }
    }
    let agent = job.agent_name.trim();
    if !agent.is_empty() {
        return agent.to_string();
    }
    job.provider.clone()
}

/// Prepare an active-mode start, loading a session and ensuring its pane.
///
/// Mirrors Python `prepare_active_start`.  If the context, session, pane or
/// backend are unavailable, returns an error `ProviderSubmission` that can be
/// handed back to the execution service.
#[allow(clippy::too_many_arguments)]
pub fn prepare_active_start<S>(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    provider: &str,
    source_kind: CompletionSourceKind,
    now: &str,
    missing_session_reason: &str,
    load_session_fn: impl FnOnce(&Path, &str) -> Option<S>,
    backend_for_session_fn: impl FnOnce(&Value) -> Option<Value>,
) -> StartOutcome<S>
where
    S: ActiveSession,
{
    let Some(context) = context else {
        return StartOutcome::Error(error_submission(
            job,
            provider,
            now,
            source_kind,
            "runtime_unavailable",
            "missing_runtime_context",
        ));
    };
    let Some(workspace_path) = context.workspace_path.as_deref() else {
        return StartOutcome::Error(error_submission(
            job,
            provider,
            now,
            source_kind,
            "runtime_unavailable",
            "missing_runtime_context",
        ));
    };
    if workspace_path.trim().is_empty() {
        return StartOutcome::Error(error_submission(
            job,
            provider,
            now,
            source_kind,
            "runtime_unavailable",
            "missing_runtime_context",
        ));
    }

    let work_dir = expand_tilde(workspace_path);
    let Some(session) = load_session_fn(&work_dir, &session_selector_name(job)) else {
        return StartOutcome::Error(error_submission(
            job,
            provider,
            now,
            source_kind,
            "runtime_unavailable",
            missing_session_reason,
        ));
    };

    let pane_id = match session.ensure_pane() {
        Ok(pane) => pane,
        Err(err) => {
            return StartOutcome::Error(error_submission(
                job,
                provider,
                now,
                source_kind,
                "pane_unavailable",
                &err,
            ));
        }
    };

    let Some(backend) = backend_for_session_fn(session.session_data()) else {
        return StartOutcome::Error(error_submission(
            job,
            provider,
            now,
            source_kind,
            "backend_unavailable",
            "terminal backend not available",
        ));
    };

    StartOutcome::Prepared(PreparedActiveStart::new(
        work_dir, session, pane_id, backend,
    ))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home + rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_completion::models::CompletionStatus;

    #[derive(Debug)]
    struct FakeSession {
        data: Value,
        pane: Result<String, String>,
    }

    impl ActiveSession for FakeSession {
        fn ensure_pane(&self) -> Result<String, String> {
            self.pane.clone()
        }

        fn session_data(&self) -> &Value {
            &self.data
        }
    }

    fn job() -> JobRecord {
        JobRecord::new("job_1", "agent1", "codex")
    }

    fn context(workspace_path: &str) -> ProviderRuntimeContext {
        ProviderRuntimeContext {
            agent_name: "agent1".to_string(),
            workspace_path: Some(workspace_path.to_string()),
            backend_type: Some("tmux".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_session_selector_name_uses_provider_instance() {
        let mut j = job();
        j.provider_instance = Some("instA".to_string());
        assert_eq!(session_selector_name(&j), "instA");
    }

    #[test]
    fn test_session_selector_name_falls_back_to_agent_name() {
        let j = job();
        assert_eq!(session_selector_name(&j), "agent1");
    }

    #[test]
    fn test_session_selector_name_falls_back_to_provider() {
        let mut j = JobRecord::new("job_1", "", "codex");
        j.provider_instance = None;
        assert_eq!(session_selector_name(&j), "codex");
    }

    #[test]
    fn test_prepare_active_start_missing_context_returns_error() {
        let outcome = prepare_active_start(
            &job(),
            None,
            "codex",
            CompletionSourceKind::SessionEventLog,
            "now",
            "missing",
            |_path, _name| -> Option<FakeSession> { None },
            |_data| None,
        );
        let StartOutcome::Error(sub) = outcome else {
            panic!("expected error submission");
        };
        assert_eq!(sub.status, CompletionStatus::Incomplete);
        assert_eq!(sub.reason, "in_progress");
        assert_eq!(
            sub.runtime_state.get("error").unwrap().as_str().unwrap(),
            "missing_runtime_context"
        );
    }

    #[test]
    fn test_prepare_active_start_missing_session_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let outcome = prepare_active_start(
            &job(),
            Some(&context(&tmp.path().to_string_lossy())),
            "codex",
            CompletionSourceKind::SessionEventLog,
            "now",
            "no_session",
            |_path, _name| -> Option<FakeSession> { None },
            |_data| None,
        );
        let StartOutcome::Error(sub) = outcome else {
            panic!("expected error submission");
        };
        assert_eq!(
            sub.runtime_state.get("error").unwrap().as_str().unwrap(),
            "no_session"
        );
    }

    #[test]
    fn test_prepare_active_start_dead_pane_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let outcome = prepare_active_start(
            &job(),
            Some(&context(&tmp.path().to_string_lossy())),
            "codex",
            CompletionSourceKind::SessionEventLog,
            "now",
            "missing",
            |_path, _name| {
                Some(FakeSession {
                    data: Value::Null,
                    pane: Err("pane dead".to_string()),
                })
            },
            |_data| None,
        );
        let StartOutcome::Error(sub) = outcome else {
            panic!("expected error submission");
        };
        assert_eq!(
            sub.runtime_state.get("error").unwrap().as_str().unwrap(),
            "pane dead"
        );
    }

    #[test]
    fn test_prepare_active_start_success() {
        let tmp = tempfile::tempdir().unwrap();
        let outcome = prepare_active_start(
            &job(),
            Some(&context(&tmp.path().to_string_lossy())),
            "codex",
            CompletionSourceKind::SessionEventLog,
            "now",
            "missing",
            |_path, _name| {
                Some(FakeSession {
                    data: serde_json::json!({"x": 1}),
                    pane: Ok("%9".to_string()),
                })
            },
            |data| Some(data.clone()),
        );
        let prepared = outcome.expect_prepared();
        assert_eq!(prepared.pane_id, "%9");
        assert_eq!(prepared.backend, serde_json::json!({"x": 1}));
    }
}
