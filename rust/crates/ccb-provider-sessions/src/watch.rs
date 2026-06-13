use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

/// Whether a file-watching backend is available. The current implementation uses
/// a polling fallback, so this is always `true`.
pub const HAS_WATCHDOG: bool = true;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Watches a project directory for created or modified session files.
///
/// Mirrors Python `provider_sessions.watch.SessionFileWatcher`. The Rust version
/// uses a polling fallback so it works without the `notify` crate.
pub struct SessionFileWatcher {
    project_dir: PathBuf,
    callback: Arc<dyn Fn(PathBuf) + Send + Sync + 'static>,
    predicate: Arc<dyn Fn(&Path) -> bool + Send + Sync + 'static>,
    recursive: bool,
    running: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl SessionFileWatcher {
    pub fn new<C>(project_dir: PathBuf, callback: C, recursive: bool) -> Self
    where
        C: Fn(PathBuf) + Send + Sync + 'static,
    {
        Self::with_predicate(project_dir, callback, recursive, None::<fn(&Path) -> bool>)
    }

    pub fn with_predicate<P, C>(
        project_dir: PathBuf,
        callback: C,
        recursive: bool,
        predicate: Option<P>,
    ) -> Self
    where
        P: Fn(&Path) -> bool + Send + Sync + 'static,
        C: Fn(PathBuf) + Send + Sync + 'static,
    {
        let predicate: Arc<dyn Fn(&Path) -> bool + Send + Sync + 'static> = match predicate {
            Some(p) => Arc::new(p),
            None => Arc::new(is_watch_file),
        };
        Self {
            project_dir,
            callback: Arc::new(callback),
            predicate,
            recursive,
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }

    pub fn start(&mut self) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let project_dir = self.project_dir.clone();
        let callback = self.callback.clone();
        let predicate = self.predicate.clone();
        let recursive = self.recursive;
        let running = self.running.clone();
        let handle = thread::spawn(move || {
            watcher_loop(project_dir, callback, predicate, recursive, running)
        });
        *self.handle.lock().unwrap() = Some(handle);
    }

    pub fn stop(&mut self) {
        if !self.running.swap(false, Ordering::SeqCst) {
            return;
        }
        if let Some(handle) = self.handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SessionFileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

fn watcher_loop(
    project_dir: PathBuf,
    callback: Arc<dyn Fn(PathBuf) + Send + Sync + 'static>,
    predicate: Arc<dyn Fn(&Path) -> bool + Send + Sync + 'static>,
    recursive: bool,
    running: Arc<AtomicBool>,
) {
    let mut snapshot: HashMap<PathBuf, (SystemTime, u64)> = HashMap::new();
    let _ = scan_dir(&project_dir, recursive, &predicate, &mut snapshot);

    while running.load(Ordering::SeqCst) {
        thread::sleep(POLL_INTERVAL);
        let mut current: HashMap<PathBuf, (SystemTime, u64)> = HashMap::new();
        let _ = scan_dir(&project_dir, recursive, &predicate, &mut current);

        for (path, state) in &current {
            let emit = match snapshot.get(path) {
                Some(prev) => prev != state,
                None => true,
            };
            if emit {
                callback(path.clone());
            }
        }
        snapshot = current;
    }
}

fn scan_dir(
    dir: &Path,
    recursive: bool,
    predicate: &Arc<dyn Fn(&Path) -> bool + Send + Sync + 'static>,
    out: &mut HashMap<PathBuf, (SystemTime, u64)>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if recursive {
                let _ = scan_dir(&path, recursive, predicate, out);
            }
            continue;
        }
        if predicate(&path) {
            if let Ok(meta) = entry.metadata() {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                out.insert(path, (mtime, meta.len()));
            }
        }
    }
    Ok(())
}

pub fn is_watch_file(path: &Path) -> bool {
    is_log_file(path) || is_index_file(path)
}

fn is_log_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("jsonl")
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| !n.starts_with('.'))
            .unwrap_or(false)
}

fn is_index_file(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some("sessions-index.json")
}
