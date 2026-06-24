//! Mirrors Python `lib/ccbrd/runtime.py`.
//! Daemon runtime utilities (work dir, log path, tokens).

use std::path::PathBuf;

pub fn get_daemon_work_dir(layout: &ccbr_storage::paths::PathLayout) -> PathBuf {
    layout.ccbrd_dir().to_path_buf()
}

pub fn log_path(layout: &ccbr_storage::paths::PathLayout) -> PathBuf {
    layout.ccbrd_dir().join("ccbrd.log")
}

pub fn state_file_path(layout: &ccbr_storage::paths::PathLayout) -> PathBuf {
    layout.ccbrd_dir().join("state.json")
}

pub fn run_dir(layout: &ccbr_storage::paths::PathLayout) -> PathBuf {
    layout.ccbrd_dir().join("run")
}

pub fn normalize_connect_host(host: &str) -> String {
    host.trim().trim_start_matches("unix://").to_string()
}

pub fn random_token() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{now:x}")
}

pub fn write_log(layout: &ccbr_storage::paths::PathLayout, message: &str) {
    let path = log_path(layout);
    let timestamp = chrono::Utc::now().to_rfc3339();
    let line = format!("[{timestamp}] {message}\n");
    let _ = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}
