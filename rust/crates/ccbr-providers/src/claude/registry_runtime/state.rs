//! Mirrors Python `lib/provider_backends/claude/registry_runtime/state.py`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A session held in the Claude registry.
/// Mirrors Python `SessionEntry`.
#[derive(Debug, Clone)]
pub struct SessionEntry<S: RegistrySession> {
    pub work_dir: PathBuf,
    pub session: Option<S>,
    pub session_file: Option<PathBuf>,
    pub file_mtime: f64,
    pub last_check: f64,
    pub valid: bool,
    pub next_bind_refresh: f64,
    pub bind_backoff_s: f64,
}

impl<S: RegistrySession> SessionEntry<S> {
    pub fn new(work_dir: PathBuf, session: S) -> Self {
        Self {
            work_dir,
            session: Some(session),
            session_file: None,
            file_mtime: 0.0,
            last_check: 0.0,
            valid: false,
            next_bind_refresh: 0.0,
            bind_backoff_s: 0.0,
        }
    }
}

/// A file watcher bucket and its associated registry keys.
/// Mirrors Python `WatcherEntry`.
#[derive(Debug, Default)]
pub struct WatcherEntry<W = ()> {
    pub watcher: W,
    pub keys: HashSet<String>,
}

/// Shared runtime state for the Claude session registry.
/// Mirrors Python `RegistryRuntimeState`.
#[derive(Debug)]
pub struct RegistryRuntimeState<S: RegistrySession, W = ()> {
    pub sessions: HashMap<String, SessionEntry<S>>,
    pub watchers: HashMap<String, WatcherEntry<W>>,
    pub pending_logs: HashMap<String, f64>,
    pub log_last_check: HashMap<String, f64>,
}

impl<S: RegistrySession, W: Default> Default for RegistryRuntimeState<S, W> {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            watchers: HashMap::new(),
            pending_logs: HashMap::new(),
            log_last_check: HashMap::new(),
        }
    }
}

/// Trait for sessions stored in the registry.
pub trait RegistrySession: Send + Sync + Clone + std::fmt::Debug {
    fn session_file(&self) -> Option<&Path>;
    fn ensure_pane(&self) -> Result<String, String>;
}

/// Thread-safe Claude registry runtime container.
#[derive(Debug)]
pub struct ClaudeRuntimeRegistry<S: RegistrySession, W = ()> {
    pub state: Arc<Mutex<RegistryRuntimeState<S, W>>>,
    pub claude_root: PathBuf,
}

impl<S: RegistrySession, W: Default> ClaudeRuntimeRegistry<S, W> {
    pub fn new(claude_root: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(RegistryRuntimeState::default())),
            claude_root,
        }
    }
}
