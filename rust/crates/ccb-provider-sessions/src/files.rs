use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(all(unix, not(target_os = "linux")))]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub session_id: String,
    pub provider: String,
    pub agent_name: String,
    pub path: PathBuf,
    pub pane_id: Option<String>,
    pub start_cmd: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub size_bytes: u64,
}

impl SessionFile {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "session_id": self.session_id,
            "provider": self.provider,
            "agent_name": self.agent_name,
            "path": self.path.to_string_lossy(),
            "pane_id": self.pane_id,
            "start_cmd": self.start_cmd,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "size_bytes": self.size_bytes,
        })
    }
}

pub struct SessionFileManager {
    layout: ccb_storage::paths::PathLayout,
}

impl SessionFileManager {
    pub fn new(layout: ccb_storage::paths::PathLayout) -> Self {
        Self { layout }
    }

    pub fn session_dir(&self, agent_name: &str) -> camino::Utf8PathBuf {
        self.layout.agent_dir(agent_name)
    }

    pub fn discover_sessions(&self, agent_name: &str) -> Vec<SessionFile> {
        let dir = self.session_dir(agent_name);
        if !dir.exists() {
            return Vec::new();
        }
        let mut sessions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(session) = serde_json::from_str::<SessionFile>(&content) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
        sessions
    }

    pub fn save_session(&self, session: &SessionFile) -> Result<(), String> {
        let dir = self.session_dir(&session.agent_name);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", session.session_id));
        let json = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }

    pub fn load_session(&self, agent_name: &str, session_id: &str) -> Option<SessionFile> {
        let dir = self.session_dir(agent_name);
        let path = dir.join(format!("{}.json", session_id));
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn delete_session(&self, agent_name: &str, session_id: &str) -> Result<(), String> {
        let dir = self.session_dir(agent_name);
        let path = dir.join(format!("{}.json", session_id));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Provider client spec (mirrors Python `provider_core.runtime_specs`).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ProviderClientSpec {
    pub provider_key: String,
    pub enabled_env: String,
    pub autostart_env: String,
    pub state_file_env: String,
    pub session_filename: String,
}

impl ProviderClientSpec {
    pub fn new(provider_key: &str, session_filename: &str) -> Self {
        let stem = provider_key.trim().to_uppercase().replace('-', "_");
        Self {
            provider_key: provider_key.to_string(),
            enabled_env: format!("CCB_{stem}"),
            autostart_env: format!("CCB_{stem}_AUTOSTART"),
            state_file_env: format!("CCB_{stem}_STATE_FILE"),
            session_filename: session_filename.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Project config / session file helpers (mirrors Python `provider_sessions`).
// ---------------------------------------------------------------------------

pub const CCB_PROJECT_CONFIG_DIRNAME: &str = ".ccbr";

const CCB_DIRNAME: &str = ".ccbr";
const WORKSPACE_BINDING_FILENAME: &str = ".ccbr-workspace.json";

pub fn project_config_dir(work_dir: impl AsRef<Path>) -> PathBuf {
    resolve_dir(work_dir.as_ref()).join(CCB_DIRNAME)
}

pub fn resolve_project_config_dir(work_dir: impl AsRef<Path>) -> PathBuf {
    project_config_dir(work_dir)
}

#[derive(Debug, Clone)]
pub struct WritableCheck {
    pub writable: bool,
    pub reason: Option<String>,
    pub fix: Option<String>,
}

pub fn check_session_writable<P: AsRef<Path>>(session_file: P) -> WritableCheck {
    let session_file = session_file.as_ref();
    let parent = session_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if !parent.exists() {
        return WritableCheck {
            writable: false,
            reason: Some(format!("Directory not found: {}", parent.display())),
            fix: Some(format!("mkdir -p {}", parent.display())),
        };
    }

    if !parent.is_dir() {
        return WritableCheck {
            writable: false,
            reason: Some(format!("Not a directory: {}", parent.display())),
            fix: Some(format!("mkdir -p {}", parent.display())),
        };
    }

    if !access_ok(parent, libc::X_OK) {
        return WritableCheck {
            writable: false,
            reason: Some(format!(
                "Directory not accessible (missing x permission): {}",
                parent.display()
            )),
            fix: Some(format!("chmod +x {}", parent.display())),
        };
    }

    if !access_ok(parent, libc::W_OK) {
        return WritableCheck {
            writable: false,
            reason: Some(format!("Directory not writable: {}", parent.display())),
            fix: Some(format!("chmod u+w {}", parent.display())),
        };
    }

    if !session_file.exists() {
        return WritableCheck {
            writable: true,
            reason: None,
            fix: None,
        };
    }

    if session_file.is_symlink() {
        let target = session_file
            .read_link()
            .unwrap_or_else(|_| PathBuf::from("?"));
        return WritableCheck {
            writable: false,
            reason: Some(format!("Is symlink pointing to {}", target.display())),
            fix: Some(format!("rm -f {}", session_file.display())),
        };
    }

    if session_file.is_dir() {
        return WritableCheck {
            writable: false,
            reason: Some("Is directory, not file".to_string()),
            fix: Some(format!(
                "rmdir {} or rm -rf {}",
                session_file.display(),
                session_file.display()
            )),
        };
    }

    if !session_file.is_file() {
        return WritableCheck {
            writable: false,
            reason: Some("Not a regular file".to_string()),
            fix: Some(format!("rm -f {}", session_file.display())),
        };
    }

    if let Some(reason) = ownership_problem(session_file) {
        return WritableCheck {
            writable: false,
            reason: Some(reason),
            fix: Some(format!(
                "sudo chown {}:{name} {}",
                current_user_name().unwrap_or_else(|| "?".to_string()),
                session_file.display(),
                name = current_user_name().unwrap_or_else(|| "?".to_string())
            )),
        };
    }

    if !file_writable(session_file) {
        let mode = session_file
            .metadata()
            .map(|m| format_mode(m.permissions().mode()))
            .unwrap_or_else(|_| "unknown".to_string());
        return WritableCheck {
            writable: false,
            reason: Some(format!("File not writable (mode: {mode})")),
            fix: Some(format!("chmod u+w {}", session_file.display())),
        };
    }

    WritableCheck {
        writable: true,
        reason: None,
        fix: None,
    }
}

pub fn safe_write_session<P: AsRef<Path>>(session_file: P, content: &str) -> Result<(), String> {
    let session_file = session_file.as_ref();
    let check = check_session_writable(session_file);
    if !check.writable {
        let reason = check.reason.unwrap_or_default();
        let fix = check.fix.unwrap_or_default();
        return Err(format!(
            "❌ Cannot write {}: {reason}\n💡 Fix: {fix}",
            session_file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("session"),
        ));
    }

    let utf8_path = camino::Utf8Path::from_path(session_file)
        .ok_or_else(|| "non-UTF-8 session path".to_string())?;

    ccb_storage::atomic::atomic_write_text(utf8_path, content).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "❌ Cannot write {}: {e}\n💡 Try: rm -f {} then retry",
                session_file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("session"),
                session_file.display()
            )
        } else {
            format!("❌ Write failed: {e}")
        }
    })
}

pub fn print_session_error(msg: &str, to_stderr: bool) {
    if to_stderr {
        eprintln!("{msg}");
    } else {
        println!("{msg}");
    }
}

/// Convenience wrapper that prints a session error to stderr.
pub fn print_session_error_stderr(msg: &str) {
    print_session_error(msg, true);
}

pub fn find_project_session_file<P: AsRef<Path>>(
    work_dir: P,
    session_filename: &str,
) -> Option<PathBuf> {
    let current = resolve_dir(work_dir.as_ref());

    if let Some(candidate) = session_file_from_workspace_binding(&current, session_filename) {
        return Some(candidate);
    }

    let anchor = find_nearest_project_anchor(&current)?;
    let candidate = project_config_dir(&anchor).join(session_filename);
    candidate.exists().then_some(candidate)
}

/// Find a session file honoring workspace binding agent names and the
/// `CCB_SESSION_FILE` environment variable.
///
/// Mirrors Python `provider_core.session_binding_runtime.find_bound_session_file`.
pub fn find_bound_session_file<P: AsRef<Path>>(
    work_dir: P,
    _provider: &str,
    base_filename: &str,
) -> Option<PathBuf> {
    let current = resolve_dir(work_dir.as_ref());

    if let Some(env_path) = env_bound_session_file(base_filename) {
        return Some(env_path);
    }

    if let Some(agent_name) = workspace_binding_agent_name(&current) {
        let agent_filename = session_filename_for_instance(base_filename, &agent_name);
        if let Some(candidate) = find_project_session_file(&current, &agent_filename) {
            return Some(candidate);
        }
    }

    find_project_session_file(&current, base_filename)
}

fn env_bound_session_file(base_filename: &str) -> Option<PathBuf> {
    let raw = std::env::var("CCB_SESSION_FILE").ok()?;
    let expanded = expand_user_path_str(&raw);
    let path = PathBuf::from(expanded);
    if !path.is_file() {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    if !session_filename_matches(base_filename, name) {
        return None;
    }
    Some(path)
}

fn session_filename_for_instance(base_filename: &str, instance: &str) -> String {
    let instance = instance.trim();
    if instance.is_empty() {
        return base_filename.to_string();
    }
    if let Some(prefix) = base_filename.strip_suffix("-session") {
        format!("{}-{}-session", prefix, instance)
    } else {
        format!("{}-{}", base_filename, instance)
    }
}

fn workspace_binding_agent_name(current: &Path) -> Option<String> {
    let binding_path = find_workspace_binding(current)?;
    let text = fs::read_to_string(&binding_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    data.get("agent_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn session_filename_matches(base: &str, name: &str) -> bool {
    if name == base {
        return true;
    }
    if let Some(prefix) = base.strip_suffix("-session") {
        name.starts_with(&format!("{}-", prefix)) && name.ends_with("-session")
    } else {
        name.starts_with(&format!("{}-", base))
    }
}

pub(crate) fn expand_user_path_str(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

fn session_file_from_workspace_binding(current: &Path, session_filename: &str) -> Option<PathBuf> {
    let binding_path = find_workspace_binding(current)?;
    let target_project = load_workspace_binding(&binding_path)?;
    let target = resolve_dir(Path::new(&target_project));
    let candidate = project_config_dir(&target).join(session_filename);
    candidate.exists().then_some(candidate)
}

fn find_workspace_binding(current: &Path) -> Option<PathBuf> {
    for root in search_roots(current) {
        let candidate = root.join(WORKSPACE_BINDING_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn load_workspace_binding(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    let target = data.get("target_project")?.as_str()?;
    if target.trim().is_empty() {
        return None;
    }
    Some(target.to_string())
}

fn find_nearest_project_anchor(current: &Path) -> Option<PathBuf> {
    let mut root = Some(current);
    while let Some(r) = root {
        if project_anchor_dir(r).is_some() {
            let dangerous = r != current && is_dangerous_project_root(r).is_some();
            if !dangerous {
                return Some(r.to_path_buf());
            }
        }
        root = r.parent();
    }
    None
}

fn project_anchor_dir(root: &Path) -> Option<PathBuf> {
    let primary = root.join(CCB_DIRNAME);
    primary.is_dir().then_some(primary)
}

fn is_dangerous_project_root(root: &Path) -> Option<&'static str> {
    if std::env::var("HOME").ok().map(PathBuf::from).as_ref() == Some(&root.to_path_buf()) {
        return Some("$HOME");
    }
    if std::env::temp_dir() == root {
        return Some("temporary directory root");
    }
    if root.parent().is_none() {
        return Some("filesystem root");
    }
    None
}

fn search_roots(current: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut cur = Some(current);
    while let Some(c) = cur {
        roots.push(c.to_path_buf());
        cur = c.parent();
    }
    roots
}

fn resolve_dir(path: &Path) -> PathBuf {
    let expanded = expand_user_path(path);
    fs::canonicalize(&expanded).unwrap_or_else(|_| {
        if expanded.is_absolute() {
            expanded.components().collect()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&expanded).components().collect())
                .unwrap_or(expanded)
        }
    })
}

fn expand_user_path(path: &Path) -> PathBuf {
    if let Some(rest) = path.as_os_str().to_str().and_then(|s| s.strip_prefix('~')) {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home + rest);
        }
    }
    path.to_path_buf()
}

#[cfg(unix)]
fn access_ok(path: &Path, mode: i32) -> bool {
    use std::ffi::CString;
    CString::new(path.as_os_str().as_bytes())
        .map(|c| unsafe { libc::access(c.as_ptr(), mode) == 0 })
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn access_ok(_path: &Path, _mode: i32) -> bool {
    true
}

#[cfg(unix)]
fn file_writable(path: &Path) -> bool {
    access_ok(path, libc::W_OK)
}

#[cfg(not(unix))]
fn file_writable(path: &Path) -> bool {
    fs::OpenOptions::new().write(true).open(path).is_ok()
}

#[cfg(unix)]
fn ownership_problem(path: &Path) -> Option<String> {
    let meta = path.metadata().ok()?;
    let file_uid = meta.st_uid();
    let current_uid = unsafe { libc::getuid() };
    if file_uid == current_uid {
        return None;
    }
    let owner = user_name(file_uid).unwrap_or_else(|| file_uid.to_string());
    let current = current_user_name().unwrap_or_else(|| current_uid.to_string());
    Some(format!("File owned by {owner} (current user: {current})"))
}

#[cfg(not(unix))]
fn ownership_problem(_path: &Path) -> Option<String> {
    None
}

#[cfg(unix)]
fn current_user_name() -> Option<String> {
    let uid = unsafe { libc::getuid() };
    user_name(uid)
}

#[cfg(unix)]
fn user_name(uid: u32) -> Option<String> {
    use std::ffi::CStr;
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() {
            return None;
        }
        let name_ptr = (*pw).pw_name;
        if name_ptr.is_null() {
            return None;
        }
        CStr::from_ptr(name_ptr).to_str().ok().map(String::from)
    }
}

#[cfg(unix)]
fn format_mode(mode: u32) -> String {
    let perms = mode & 0o777;
    let r = |bit: u32| if perms & bit != 0 { 'r' } else { '-' };
    let w = |bit: u32| if perms & bit != 0 { 'w' } else { '-' };
    let x = |bit: u32| if perms & bit != 0 { 'x' } else { '-' };
    format!(
        "-{}{}{}{}{}{}{}{}{}",
        r(0o400),
        w(0o200),
        x(0o100),
        r(0o040),
        w(0o020),
        x(0o010),
        r(0o004),
        w(0o002),
        x(0o001)
    )
}

#[cfg(not(unix))]
fn format_mode(_mode: u32) -> String {
    "unknown".to_string()
}
