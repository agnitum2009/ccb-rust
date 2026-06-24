//! Mirrors Python `lib/provider_execution/active_runtime/resume.py`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_completion::models::JobRecord;
use serde_json::Value;

use crate::execution::{ProviderRuntimeContext, ProviderSubmission};

use super::models::PreparedActiveStart;
use super::start::session_selector_name;

type ConfigureReaderFn<R> = dyn FnOnce(&mut R, &HashMap<String, Value>, &ProviderRuntimeContext);
type CompletionDirFn<S> = dyn FnOnce(&S) -> String;

/// Session abstraction used by the active-runtime helpers.
///
/// The concrete session types live in each provider backend; this trait lets the
/// generic resume/start code stay provider-agnostic while still being able to
/// verify that the underlying tmux pane is alive.
pub trait ActiveSession {
    /// Ensure the tmux pane for this session exists and is alive.
    ///
    /// On success returns the pane id (e.g. `%9`). On failure returns a human
    /// readable reason; the helper treats every failure as "cannot resume".
    fn ensure_pane(&self) -> Result<String, String>;

    /// Provider-specific session data used to build a terminal backend.
    fn session_data(&self) -> &Value;
}

/// Resume an active-mode submission from a persisted state record.
///
/// Mirrors Python `resume_active_submission`.  All provider-specific loading is
/// injected via callbacks so the helper stays testable without concrete tmux
/// dependencies.
#[allow(clippy::too_many_arguments)]
pub fn resume_active_submission<S, R>(
    job: &JobRecord,
    submission: &ProviderSubmission,
    context: Option<&ProviderRuntimeContext>,
    load_session_fn: impl FnOnce(&Path, &str) -> Option<S>,
    backend_for_session_fn: impl FnOnce(&Value) -> Option<Value>,
    reader_factory: impl FnOnce(&S) -> R,
    configure_reader_fn: Option<Box<ConfigureReaderFn<R>>>,
    completion_dir_fn: Option<Box<CompletionDirFn<S>>>,
) -> Option<ProviderSubmission>
where
    S: ActiveSession + 'static,
    R: std::fmt::Debug,
{
    let state = &submission.runtime_state;
    let work_dir = active_work_dir(context, state)?;

    let (session, pane_id) = resume_prepared_session(job, &work_dir, load_session_fn)?;

    let backend = backend_for_session_fn(session.session_data())?;

    let mut reader = reader_factory(&session);
    if let Some(configure) = configure_reader_fn {
        configure(
            &mut reader,
            state,
            context.expect("context validated above"),
        );
    }

    let runtime_state =
        resumed_runtime_state(state, reader, backend, pane_id, &session, completion_dir_fn);
    Some(ProviderSubmission {
        runtime_state,
        ..submission.clone()
    })
}

fn active_work_dir(
    context: Option<&ProviderRuntimeContext>,
    state: &HashMap<String, Value>,
) -> Option<PathBuf> {
    let context = context?;
    let workspace_path = context.workspace_path.as_deref()?;
    if workspace_path.trim().is_empty() {
        return None;
    }
    let mode = state
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("passive");
    if mode != "active" {
        return None;
    }
    Some(expand_tilde(workspace_path))
}

fn resume_prepared_session<S>(
    job: &JobRecord,
    work_dir: &Path,
    load_session_fn: impl FnOnce(&Path, &str) -> Option<S>,
) -> Option<(S, String)>
where
    S: ActiveSession,
{
    let session = load_session_fn(work_dir, &session_selector_name(job))?;
    let pane_id = session.ensure_pane().ok()?;
    Some((session, pane_id))
}

fn resumed_runtime_state<S, R>(
    state: &HashMap<String, Value>,
    reader: R,
    backend: Value,
    pane_id: String,
    _session: &S,
    completion_dir_fn: Option<impl FnOnce(&S) -> String>,
) -> HashMap<String, Value>
where
    S: ActiveSession,
    R: std::fmt::Debug,
{
    let mut runtime_state = state.clone();
    runtime_state.insert("reader".to_string(), Value::String(format!("{:?}", reader)));
    runtime_state.insert("backend".to_string(), backend);
    runtime_state.insert("pane_id".to_string(), Value::String(pane_id));
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    runtime_state.insert(
        "session_path".to_string(),
        state
            .get("session_path")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );

    if let Some(get_dir) = completion_dir_fn {
        let dir = state
            .get("completion_dir")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| get_dir(_session));
        runtime_state.insert("completion_dir".to_string(), Value::String(dir));
    }
    runtime_state
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home + rest);
        }
    }
    PathBuf::from(path)
}

impl<S> PreparedActiveStart<S> {
    /// Convenience constructor used by tests and provider adapters.
    pub fn new(work_dir: PathBuf, session: S, pane_id: impl Into<String>, backend: Value) -> Self {
        Self {
            work_dir,
            session,
            pane_id: pane_id.into(),
            backend,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccbr_completion::models::{CompletionSourceKind, CompletionStatus, JobRecord};

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

    fn submission(mode: &str) -> ProviderSubmission {
        let mut runtime_state = HashMap::new();
        runtime_state.insert("mode".to_string(), Value::String(mode.to_string()));
        ProviderSubmission {
            job_id: "job_1".to_string(),
            agent_name: "agent1".to_string(),
            provider: "codex".to_string(),
            accepted_at: "2026-04-07T00:00:00Z".to_string(),
            ready_at: "2026-04-07T00:00:00Z".to_string(),
            source_kind: CompletionSourceKind::SessionEventLog,
            reply: "reply".to_string(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".to_string(),
            confidence: ccbr_completion::models::CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state,
        }
    }

    fn context(tmp: &tempfile::TempDir) -> ProviderRuntimeContext {
        ProviderRuntimeContext {
            agent_name: "agent1".to_string(),
            workspace_path: Some(tmp.path().to_string_lossy().to_string()),
            backend_type: Some("tmux".to_string()),
            runtime_ref: None,
            session_ref: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_resume_active_submission_requires_active_workspace() {
        let resumed = resume_active_submission(
            &job(),
            &submission("active"),
            None,
            |_path, _name| -> Option<FakeSession> { None },
            |_data| None,
            |_session| (), // reader unit
            None::<Box<dyn FnOnce(&mut (), &HashMap<String, Value>, &ProviderRuntimeContext)>>,
            None::<Box<dyn FnOnce(&FakeSession) -> String>>,
        );
        assert!(resumed.is_none());
    }

    #[test]
    fn test_resume_active_submission_skips_passive_runtime_state() {
        let tmp = tempfile::tempdir().unwrap();
        let resumed = resume_active_submission(
            &job(),
            &submission("passive"),
            Some(&context(&tmp)),
            |_path, _name| -> Option<FakeSession> { None },
            |_data| None,
            |_session| (),
            None::<Box<dyn FnOnce(&mut (), &HashMap<String, Value>, &ProviderRuntimeContext)>>,
            None::<Box<dyn FnOnce(&FakeSession) -> String>>,
        );
        assert!(resumed.is_none());
    }

    #[test]
    fn test_resume_active_submission_restores_reader_backend_and_completion_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let session = FakeSession {
            data: serde_json::json!({"provider": "codex"}),
            pane: Ok("%9".to_string()),
        };

        type ConfiguredEntries = Vec<(String, HashMap<String, Value>, ProviderRuntimeContext)>;
        let configured: std::rc::Rc<std::cell::RefCell<ConfiguredEntries>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let configured_cb = configured.clone();

        let resumed = resume_active_submission(
            &job(),
            &submission("active"),
            Some(&context(&tmp)),
            |_path, _name| Some(session),
            |data| Some(data.clone()),
            |_session| "reader-ok".to_string(),
            Some(Box::new(
                move |reader: &mut String,
                      state: &HashMap<String, Value>,
                      ctx: &ProviderRuntimeContext| {
                    configured_cb
                        .borrow_mut()
                        .push((reader.clone(), state.clone(), ctx.clone()));
                },
            )),
            Some(Box::new(|_session| "/tmp/completions".to_string())),
        )
        .expect("expected resume to succeed");

        assert_eq!(resumed.runtime_state.get("pane_id").unwrap(), "%9");
        assert_eq!(
            resumed.runtime_state.get("backend").unwrap(),
            &serde_json::json!({"provider": "codex"})
        );
        // reader is stored via Debug formatting
        assert!(resumed
            .runtime_state
            .get("reader")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("reader-ok"));
        assert_eq!(
            resumed.runtime_state.get("completion_dir").unwrap(),
            "/tmp/completions"
        );
        let captured = configured.borrow();
        assert_eq!(captured.len(), 1);
        assert_eq!(
            captured[0].1.get("mode").unwrap().as_str().unwrap(),
            "active"
        );
    }
}
