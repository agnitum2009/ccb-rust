//! Auto-transfer orchestration for context transfer on session switch.
//! Mirrors Python `lib/memory/transfer_runtime/auto_transfer_runtime`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DEFAULT_TTL_S: u64 = 3600;
const ENV_KEY: &str = "CCBR_CTX_TRANSFER_ON_SESSION_SWITCH";

static AUTO_TRANSFER_SEEN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

/// Reset the auto-transfer deduplication map. Exposed for tests.
pub fn clear_seen() {
    let mut guard = AUTO_TRANSFER_SEEN.lock().unwrap();
    *guard = Some(HashMap::new());
}

fn seen_map() -> std::sync::MutexGuard<'static, Option<HashMap<String, Instant>>> {
    AUTO_TRANSFER_SEEN.lock().unwrap()
}

fn env_bool_default_true(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let trimmed = v.trim().to_lowercase();
            !trimmed.is_empty() && !matches!(trimmed.as_str(), "0" | "false" | "no" | "off")
        }
        Err(_) => true,
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.expanduser()
        .and_then(|p| p.canonicalize().or(Ok(p)))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn is_current_work_dir(work_dir: &Path) -> bool {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    normalize_path(&cwd) == normalize_path(work_dir)
}

fn auto_transfer_key(
    provider: &str,
    work_dir: &Path,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> String {
    format!(
        "{}::{}::{}::{}::{}",
        provider,
        work_dir.display(),
        session_path
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        session_id.unwrap_or(""),
        project_id.unwrap_or("")
    )
}

fn claim_auto_transfer(key: &str, ttl: Duration) -> bool {
    let mut guard = seen_map();
    let map = guard.get_or_insert_with(HashMap::new);
    let now = Instant::now();
    if map.contains_key(key) {
        return false;
    }
    map.retain(|_, ts| now.duration_since(*ts) < ttl);
    map.insert(key.to_string(), now);
    true
}

/// Default no-op start function. The real implementation would spawn a transfer thread.
fn default_start(
    _provider: &str,
    _work_dir: &Path,
    _session_path: Option<&Path>,
    _session_id: Option<&str>,
    _project_id: Option<&str>,
) {
}

/// Possibly start an auto-transfer for the given session, deduplicated by key.
pub fn maybe_auto_transfer(
    provider: &str,
    work_dir: &Path,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    project_id: Option<&str>,
) {
    maybe_auto_transfer_with(
        provider,
        work_dir,
        session_path,
        session_id,
        project_id,
        default_start,
    );
}

/// Variant that accepts a custom start callback, useful for tests.
pub fn maybe_auto_transfer_with<F>(
    provider: &str,
    work_dir: &Path,
    session_path: Option<&Path>,
    session_id: Option<&str>,
    project_id: Option<&str>,
    mut start_fn: F,
) where
    F: FnMut(&str, &Path, Option<&Path>, Option<&str>, Option<&str>),
{
    if !env_bool_default_true(ENV_KEY) {
        return;
    }
    if session_path.is_none() && session_id.is_none() {
        return;
    }
    let normalized_work_dir = normalize_path(work_dir);
    if !is_current_work_dir(&normalized_work_dir) {
        return;
    }
    let key = auto_transfer_key(
        provider,
        &normalized_work_dir,
        session_path,
        session_id,
        project_id,
    );
    if !claim_auto_transfer(&key, Duration::from_secs(DEFAULT_TTL_S)) {
        return;
    }
    start_fn(
        provider,
        &normalized_work_dir,
        session_path,
        session_id,
        project_id,
    );
}

trait ExpandUser {
    fn expanduser(&self) -> std::io::Result<PathBuf>;
}

impl ExpandUser for Path {
    fn expanduser(&self) -> std::io::Result<PathBuf> {
        let s = self.to_string_lossy();
        if let Some(rest) = s.strip_prefix('~') {
            if let Ok(home) = std::env::var("HOME") {
                return Ok(PathBuf::from(home + rest));
            }
        }
        Ok(self.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn auto_transfer_key_is_deterministic() {
        let key1 = auto_transfer_key(
            "codex",
            Path::new("/tmp/wd"),
            Some(Path::new("/tmp/session.json")),
            Some("sid-1"),
            Some("proj-1"),
        );
        let key2 = auto_transfer_key(
            "codex",
            Path::new("/tmp/wd"),
            Some(Path::new("/tmp/session.json")),
            Some("sid-1"),
            Some("proj-1"),
        );
        assert_eq!(key1, key2);
    }

    #[test]
    fn claim_auto_transfer_deduplicates_same_key() {
        clear_seen();
        assert!(claim_auto_transfer("k1", Duration::from_secs(60)));
        assert!(!claim_auto_transfer("k1", Duration::from_secs(60)));
        assert!(claim_auto_transfer("k2", Duration::from_secs(60)));
    }
}
