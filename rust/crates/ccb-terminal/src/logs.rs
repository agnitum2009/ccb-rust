use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::env;
use crate::panes::TmuxRunner;
use crate::tmux;

/// Manager for tmux pane log files.
pub struct TmuxPaneLogManager {
    socket_name: Option<String>,
    tmux_run: Box<dyn TmuxRunner>,
    is_alive: Box<dyn Fn(&str) -> bool + Send + Sync>,
    pane_log_info: std::sync::Mutex<HashMap<String, f64>>,
}

impl TmuxPaneLogManager {
    pub fn new<F>(socket_name: Option<String>, tmux_run: Box<dyn TmuxRunner>, is_alive: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        Self {
            socket_name,
            tmux_run,
            is_alive: Box::new(is_alive),
            pane_log_info: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn pane_log_path(&self, pane_id: &str) -> Option<PathBuf> {
        let pid = pane_id.trim();
        if pid.is_empty() {
            return None;
        }
        Some(pane_log_path_for(pid, "tmux", self.socket_name.as_deref()))
    }

    pub fn ensure_pane_log(&self, pane_id: &str) -> Option<PathBuf> {
        let pid = pane_id.trim();
        if pid.is_empty() {
            return None;
        }
        let log_path = self.pane_log_path(pid)?;
        prepare_log_path(&log_path);
        if !self.pipe_pane_output(pid, &log_path) {
            return Some(log_path);
        }
        maybe_trim_log(&log_path);
        if let Ok(mut info) = self.pane_log_info.lock() {
            info.insert(pid.to_string(), now_secs());
        }
        Some(log_path)
    }

    pub fn refresh_pane_logs(&self) {
        let Ok(info) = self.pane_log_info.lock() else {
            return;
        };
        for pid in info.keys().cloned().collect::<Vec<_>>() {
            if self.should_refresh_pane_log(&pid) {
                let _ = self.ensure_pane_log(&pid);
            }
        }
    }

    fn should_refresh_pane_log(&self, pane_id: &str) -> bool {
        (self.is_alive)(pane_id) && !self.pane_pipe_enabled(pane_id)
    }

    fn pane_pipe_enabled(&self, pane_id: &str) -> bool {
        let Ok(output) = self.tmux_run.run(
            &["display-message", "-p", "-t", pane_id, "#{pane_pipe}"],
            false,
            true,
        ) else {
            return false;
        };
        tmux::pane_pipe_enabled(&output.stdout)
    }

    fn pipe_pane_output(&self, pane_id: &str, log_path: &Path) -> bool {
        let cmd = format!("tee -a {}", log_path.display());
        self.tmux_run
            .run(&["pipe-pane", "-o", "-t", pane_id, &cmd], false, false)
            .is_ok()
    }
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn prepare_log_path(log_path: &Path) {
    cleanup_pane_logs(log_path.parent().unwrap_or_else(|| Path::new(".")));
    let _ = std::fs::create_dir_all(log_path.parent().unwrap_or_else(|| Path::new(".")));
    let _ = std::fs::File::create(log_path);
}

/// Root directory for pane logs.
pub fn pane_log_root() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache").join("ccb")
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".cache")
            .join("ccb")
    }
}

/// Directory for a backend/socket combination.
pub fn pane_log_dir(backend: &str, socket_name: Option<&str>) -> PathBuf {
    let root = pane_log_root();
    if backend == "tmux" {
        if let Some(name) = socket_name {
            let safe = env::sanitize_filename(name);
            let safe = if safe.is_empty() { "default" } else { &safe };
            return root.join(format!("tmux-{safe}"));
        }
        return root.join("tmux");
    }
    let safe = env::sanitize_filename(backend);
    let safe = if safe.is_empty() { "pane" } else { &safe };
    root.join(safe)
}

/// Path for a specific pane log.
pub fn pane_log_path_for(pane_id: &str, backend: &str, socket_name: Option<&str>) -> PathBuf {
    let pane = pane_id.trim().replace('%', "");
    let safe = env::sanitize_filename(&pane);
    let safe = if safe.is_empty() { "pane" } else { &safe };
    pane_log_dir(backend, socket_name).join(format!("pane-{safe}.log"))
}

static LAST_PANE_LOG_CLEAN: std::sync::Mutex<f64> = std::sync::Mutex::new(0.0);

/// Clean up old pane logs in a directory.
pub fn cleanup_pane_logs(dir_path: &Path) {
    let interval_s = env::env_float("CCB_PANE_LOG_CLEAN_INTERVAL_S", 600.0);
    let now = now_secs();
    {
        let Ok(mut last) = LAST_PANE_LOG_CLEAN.lock() else {
            return;
        };
        if interval_s > 0.0 && (now - *last) < interval_s {
            return;
        }
        *last = now;
    }

    let ttl_days = env::env_int("CCB_PANE_LOG_TTL_DAYS", 7);
    let max_files = env::env_int("CCB_PANE_LOG_MAX_FILES", 200);
    if ttl_days <= 0 && max_files <= 0 {
        return;
    }
    if !dir_path.exists() {
        return;
    }
    let mut files = list_log_files(dir_path);
    if ttl_days > 0 {
        files = drop_expired_logs(&files, now, ttl_days);
    }
    if max_files > 0 && files.len() > max_files as usize {
        trim_extra_logs(&files, max_files as usize);
    }
}

fn list_log_files(dir_path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir_path)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.is_file() {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn drop_expired_logs(files: &[PathBuf], now: f64, ttl_days: i64) -> Vec<PathBuf> {
    let cutoff = now - (ttl_days as f64 * 86400.0);
    files
        .iter()
        .filter(|path| {
            let keep = path
                .metadata()
                .ok()
                .map(|m| {
                    m.modified()
                        .ok()
                        .map(|t| {
                            t.duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64()
                                >= cutoff
                        })
                        .unwrap_or(true)
                })
                .unwrap_or(true);
            if !keep {
                let _ = std::fs::remove_file(path);
            }
            keep
        })
        .cloned()
        .collect()
}

fn trim_extra_logs(files: &[PathBuf], max_files: usize) {
    let mut files: Vec<_> = files.to_vec();
    files.sort_by(|a, b| {
        let ta = a.metadata().ok().and_then(|m| m.modified().ok());
        let tb = b.metadata().ok().and_then(|m| m.modified().ok());
        ta.cmp(&tb)
    });
    let extra = files.len().saturating_sub(max_files);
    for path in &files[..extra] {
        let _ = std::fs::remove_file(path);
    }
}

/// Trim a log file to a maximum byte size keeping the tail.
pub fn maybe_trim_log(path: &Path) {
    let max_bytes = env::env_int("CCB_PANE_LOG_MAX_BYTES", 10 * 1024 * 1024);
    if max_bytes <= 0 {
        return;
    }
    let Ok(size) = path.metadata().map(|m| m.len()) else {
        return;
    };
    if size <= max_bytes as u64 {
        return;
    }
    let Some(tail) = read_log_tail(path, max_bytes as usize) else {
        return;
    };
    replace_log_with_tail(path, &tail);
}

fn read_log_tail(path: &Path, max_bytes: usize) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    if data.len() <= max_bytes {
        Some(data)
    } else {
        Some(data[data.len() - max_bytes..].to_vec())
    }
}

fn replace_log_with_tail(path: &Path, tail: &[u8]) {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let _ = std::fs::create_dir_all(parent);
    let tmp_path = parent.join(format!(
        ".{}",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if std::fs::write(&tmp_path, tail).is_ok() {
        let _ = std::fs::rename(&tmp_path, path);
    }
    let _ = std::fs::remove_file(&tmp_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::{TmuxRunOutput, TmuxRunner};

    fn ok() -> TmuxRunOutput {
        TmuxRunOutput {
            stdout: String::new(),
            stderr: String::new(),
            returncode: 0,
        }
    }

    #[test]
    fn test_pane_log_path_for() {
        let path = pane_log_path_for("%42", "tmux", Some("demo"));
        assert!(path.to_string_lossy().contains("tmux-demo"));
        assert!(path.to_string_lossy().contains("pane-42.log"));
    }

    #[test]
    fn test_tmux_pane_log_manager_ensures_log_and_tracks_info() {
        let tmp_dir =
            std::env::temp_dir().join(format!("ccb-terminal-logs-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp_dir);
        let _log_path = tmp_dir.join("%1.log");
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
        let calls_clone = calls.clone();
        let runner: Box<dyn TmuxRunner> =
            Box::new(move |args: &[&str], _check: bool, _capture: bool| {
                calls_clone
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                Ok(ok())
            });
        let manager = TmuxPaneLogManager::new(Some("sock".to_string()), runner, |_pane_id| true);
        let _ = manager.ensure_pane_log("%1");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][0], "pipe-pane");
        assert_eq!(calls[0][1], "-o");
        assert_eq!(calls[0][2], "-t");
        assert_eq!(calls[0][3], "%1");
        assert!(calls[0][4].starts_with("tee -a"));
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_cleanup_pane_logs_drops_expired() {
        let tmp = std::env::temp_dir().join(format!("ccb-log-clean-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let old = tmp.join("old.log");
        std::fs::write(&old, "x").unwrap();
        let _ = std::process::Command::new("touch")
            .arg("-d")
            .arg("1970-01-01")
            .arg(&old)
            .status();
        std::env::set_var("CCB_PANE_LOG_CLEAN_INTERVAL_S", "0");
        std::env::set_var("CCB_PANE_LOG_TTL_DAYS", "1");
        cleanup_pane_logs(&tmp);
        assert!(!old.exists());
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::remove_var("CCB_PANE_LOG_CLEAN_INTERVAL_S");
        std::env::remove_var("CCB_PANE_LOG_TTL_DAYS");
    }
}
