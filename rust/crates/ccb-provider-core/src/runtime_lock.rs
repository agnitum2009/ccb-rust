use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use fs2::FileExt;

/// Per-provider, per-scope file lock used to serialize runtime request-response
/// cycles.
pub struct ProviderLock {
    pub provider: String,
    pub timeout: Duration,
    pub lock_dir: PathBuf,
    pub lock_file: PathBuf,
    file: Option<File>,
    acquired: bool,
}

impl ProviderLock {
    /// Create a new provider lock scoped to `cwd` (or the current working
    /// directory if `None`).
    pub fn new(provider: &str, timeout_seconds: f64, cwd: Option<&Path>) -> Self {
        let provider = provider.trim().to_lowercase();
        let scope = cwd
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let lock_dir = lock_dir_for_scope(&scope);
        let scope_hash = format!("{:x}", md5_hash(scope.to_string_lossy().as_bytes()));
        let scope_short = &scope_hash[..scope_hash.len().min(8)];
        let lock_file = lock_dir.join(format!("{}-{}.lock", provider, scope_short));
        Self {
            provider,
            timeout: Duration::from_secs_f64(timeout_seconds.max(0.0)),
            lock_dir,
            lock_file,
            file: None,
            acquired: false,
        }
    }

    /// Try to acquire the lock once without blocking.
    pub fn try_acquire(&mut self) -> bool {
        self.lock_dir.mkdirs();
        let file = match open_lock_file(&self.lock_file) {
            Some(f) => f,
            None => return false,
        };
        self.file = Some(file);
        if self.try_acquire_once() {
            return true;
        }
        if self.check_stale_lock() {
            self.reopen_and_try()
        } else {
            self.file = None;
            false
        }
    }

    /// Acquire the lock, blocking up to `self.timeout`.
    pub fn acquire(&mut self) -> bool {
        self.lock_dir.mkdirs();
        let file = match open_lock_file(&self.lock_file) {
            Some(f) => f,
            None => return false,
        };
        self.file = Some(file);

        let deadline = Instant::now() + self.timeout;
        let mut stale_checked = false;

        while Instant::now() < deadline {
            if self.try_acquire_once() {
                return true;
            }
            if !stale_checked {
                stale_checked = true;
                if self.check_stale_lock() && self.reopen_and_try() {
                    return true;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        self.file = None;
        false
    }

    /// Release the lock.
    pub fn release(&mut self) {
        if let Some(file) = self.file.take() {
            if self.acquired {
                let _ = file.unlock();
            }
            self.acquired = false;
        }
    }

    fn try_acquire_once(&mut self) -> bool {
        let file = match self.file.as_ref() {
            Some(f) => f,
            None => return false,
        };
        match file.try_lock_exclusive() {
            Ok(()) => {
                if let Err(e) = write_pid(file) {
                    tracing::warn!("failed to write pid to lock file: {e}");
                }
                self.acquired = true;
                true
            }
            Err(_) => false,
        }
    }

    fn check_stale_lock(&self) -> bool {
        let pid = match read_pid(&self.lock_file) {
            Some(p) => p,
            None => return false,
        };
        if is_pid_alive(pid) {
            return false;
        }
        let _ = std::fs::remove_file(&self.lock_file);
        true
    }

    fn reopen_and_try(&mut self) -> bool {
        self.file = None;
        self.acquired = false;
        let file = match open_lock_file(&self.lock_file) {
            Some(f) => f,
            None => return false,
        };
        self.file = Some(file);
        self.try_acquire_once()
    }
}

impl Drop for ProviderLock {
    fn drop(&mut self) {
        self.release();
    }
}

fn open_lock_file(path: &Path) -> Option<File> {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(path)
        .ok()
}

fn write_pid(file: &File) -> std::io::Result<()> {
    let pid = std::process::id();
    let mut file = file;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(format!("{}\n", pid).as_bytes())?;
    file.set_len(format!("{}\n", pid).len() as u64)?;
    file.sync_all()?;
    Ok(())
}

fn read_pid(path: &Path) -> Option<u32> {
    let mut text = String::new();
    let mut file = File::open(path).ok()?;
    file.read_to_string(&mut text).ok()?;
    text.trim().parse().ok()
}

#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn is_pid_alive(_pid: u32) -> bool {
    // On non-Unix platforms, assume the lock owner is alive.
    true
}

fn lock_dir_for_scope(_scope: &Path) -> PathBuf {
    // For now, use a runtime-wide lock directory. In a full port this would
    // check for a project anchor and use the project lock dir instead.
    let runtime_root = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    runtime_root.join("ccb-runtime").join("locks")
}

fn md5_hash(data: &[u8]) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish() as u128
}

trait Mkdirs {
    fn mkdirs(&self);
}

impl Mkdirs for PathBuf {
    fn mkdirs(&self) {
        let _ = std::fs::create_dir_all(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_lock_acquire_and_release() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut lock = ProviderLock::new("claude", 5.0, Some(tmp.path()));
        assert!(lock.acquire());
        lock.release();
    }

    #[test]
    fn test_provider_lock_try_acquire() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut lock = ProviderLock::new("claude", 5.0, Some(tmp.path()));
        assert!(lock.try_acquire());
        lock.release();
    }
}
