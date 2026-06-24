//! Mirrors Python `lib/cli/services/cleanup.py`.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use ccbr_providers::execution::state_store::ExecutionStateStore;
use ccbr_storage::locks::FileLock;

use crate::context::CliContext;

const PENDING_JOB_STATUSES: &[&str] = &["accepted", "queued", "running"];
const PANE_CRASH_LOG_MAX_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;
const PANE_CRASH_LOG_MAX_KEEP_PER_RUNTIME: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupAction {
    pub provider: String,
    pub kind: String,
    pub path: String,
    pub bytes_removed: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupSkipped {
    pub provider: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CleanupSummary {
    pub project_root: String,
    pub project_id: String,
    pub status: String,
    pub deleted_bytes: u64,
    pub deleted_count: usize,
    pub skipped_count: usize,
    #[serde(default)]
    pub actions: Vec<CleanupAction>,
    #[serde(default)]
    pub skipped: Vec<CleanupSkipped>,
}

/// Result of inspecting the daemon state.
#[derive(Debug, Clone, Default)]
pub struct DaemonInspection {
    pub pid_alive: bool,
    pub socket_connectable: bool,
    pub phase: String,
    pub desired_state: String,
    pub mounted: bool,
}

/// Pluggable daemon inspector so tests can avoid monkeypatching.
pub trait DaemonInspector: Send + Sync {
    fn inspect_daemon(&self, context: &CliContext) -> Result<DaemonInspection>;
}

/// Default inspector that assumes the daemon is stopped and unmounted.
#[derive(Debug, Clone, Default)]
pub struct DefaultDaemonInspector;

impl DaemonInspector for DefaultDaemonInspector {
    fn inspect_daemon(&self, _context: &CliContext) -> Result<DaemonInspection> {
        Ok(DaemonInspection::default())
    }
}

/// Run cleanup using the default daemon inspector.
pub fn cleanup_project_storage(context: &CliContext, command: &Value) -> Result<CleanupSummary> {
    cleanup_project_storage_with(context, command, &DefaultDaemonInspector)
}

/// Run cleanup with a custom daemon inspector (used by tests).
pub fn cleanup_project_storage_with<I: DaemonInspector>(
    context: &CliContext,
    _command: &Value,
    inspector: &I,
) -> Result<CleanupSummary> {
    let lock_path = context.paths.ccbrd_dir().join("startup.lock");
    let _lock = FileLock::acquire(&lock_path)?;

    _require_stopped_backend(context, inspector)?;
    _require_no_pending_jobs(context)?;

    let mut actions = Vec::new();
    let mut skipped = Vec::new();

    _cleanup_claude_version_caches(&context.paths, &mut actions, &mut skipped)?;
    _cleanup_claude_rebuildable_caches(&context.paths, &mut actions, &mut skipped)?;
    _cleanup_gemini_rebuildable_caches(&context.paths, &mut actions, &mut skipped)?;
    _cleanup_pane_crash_logs(&context.paths, &mut actions, &mut skipped)?;

    let deleted_bytes = actions.iter().map(|a| a.bytes_removed).sum();

    Ok(CleanupSummary {
        project_root: context.project.project_root.to_string_lossy().to_string(),
        project_id: context.project.project_id.clone(),
        status: "ok".into(),
        deleted_bytes,
        deleted_count: actions.len(),
        skipped_count: skipped.len(),
        actions,
        skipped,
    })
}

fn _require_stopped_backend(context: &CliContext, inspector: &impl DaemonInspector) -> Result<()> {
    let inspection = inspector.inspect_daemon(context)?;
    if inspection.pid_alive || inspection.socket_connectable {
        bail!("ccbr cleanup requires stopped ccbrd; run `ccbr kill` first");
    }
    let phase = inspection.phase.trim();
    if !phase.is_empty() && phase != "unmounted" && phase != "failed" {
        bail!("ccbr cleanup requires stopped ccbrd; current phase={phase}");
    }
    let desired = inspection.desired_state.trim();
    if !desired.is_empty() && desired != "stopped" {
        bail!("ccbr cleanup requires stopped ccbrd; desired_state={desired}");
    }
    Ok(())
}

fn _require_no_pending_jobs(context: &CliContext) -> Result<()> {
    let store = ExecutionStateStore::new(context.paths.clone());
    let summary = store.summary();
    let active_execution_count = summary
        .get("active_execution_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let pending_items_count = summary
        .get("pending_items_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let terminal_pending_count = summary
        .get("terminal_pending_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let pending_job_count = _pending_job_count(&context.paths)?;

    if active_execution_count > 0
        || pending_items_count > 0
        || terminal_pending_count > 0
        || pending_job_count > 0
    {
        bail!("ccbr cleanup refused: pending ask jobs exist; wait for completion or run `ccbr kill` after terminalization");
    }
    Ok(())
}

fn _pending_job_count(layout: &ccbr_storage::paths::PathLayout) -> Result<u64> {
    let mut count: u64 = 0;
    let roots = [layout.agents_dir(), layout.ccbrd_dir().join("targets")];
    for root in roots {
        if !root.exists() {
            continue;
        }
        for path in _rglob_files(&root, "jobs.jsonl")? {
            count += _pending_job_count_in_file(&path)? as u64;
        }
    }
    Ok(count)
}

fn _pending_job_count_in_file(path: &Utf8Path) -> Result<usize> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(1),
    };
    let reader = BufReader::new(file);
    let mut latest_by_job: BTreeMap<String, String> = BTreeMap::new();
    let mut malformed_count: usize = 0;

    for line in reader.lines() {
        let text = line.unwrap_or_default().trim().to_string();
        if text.is_empty() {
            continue;
        }
        let record: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => {
                malformed_count += 1;
                continue;
            }
        };
        let Some(record) = record.as_object() else {
            malformed_count += 1;
            continue;
        };
        let job_id = record
            .get("job_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if job_id.is_empty() {
            continue;
        }
        let status = record
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        latest_by_job.insert(job_id, status);
    }

    let pending = latest_by_job
        .values()
        .filter(|s| PENDING_JOB_STATUSES.contains(&s.as_str()))
        .count();
    Ok(pending + malformed_count)
}

fn _rglob_files(root: &Utf8Path, name: &str) -> Result<Vec<Utf8PathBuf>> {
    let mut matches = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(path) = Utf8PathBuf::from_path_buf(path) else {
                continue;
            };
            if path.file_name() == Some(name) && path.is_file() {
                matches.push(path);
            } else if path.is_dir() && !path.is_symlink() {
                stack.push(path);
            }
        }
    }
    matches.sort();
    Ok(matches)
}

fn _cleanup_claude_version_caches(
    layout: &ccbr_storage::paths::PathLayout,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    let agents_dir = layout.agents_dir();
    let legacy_shared_versions = layout.shared_cache_dir().join("claude").join("versions");
    let external_versions = layout
        .provider_external_cache_dir("claude")?
        .join("versions");

    let mut legacy_shared_active: HashSet<String> = HashSet::new();
    let mut external_active: HashSet<String> = HashSet::new();

    if agents_dir.exists() {
        let homes = _sorted_provider_homes(&agents_dir, "claude")?;
        for home in homes {
            let active_name = _current_claude_version_name(&home);
            let versions_dir = home.join(".local/share/claude/versions");

            if let Some(ref name) = active_name {
                if versions_dir.is_symlink() {
                    if _same_path(&versions_dir, &legacy_shared_versions) {
                        legacy_shared_active.insert(name.clone());
                    }
                    if _same_path(&versions_dir, &external_versions) {
                        external_active.insert(name.clone());
                    }
                }
            }

            let active_set: HashSet<String> = active_name.into_iter().collect();
            _cleanup_one_claude_versions_dir(&versions_dir, &active_set, actions, skipped)?;
        }
    }

    _cleanup_shared_claude_versions_dir(
        &legacy_shared_versions,
        &legacy_shared_active,
        actions,
        skipped,
    )?;
    _cleanup_shared_claude_versions_dir(&external_versions, &external_active, actions, skipped)?;

    Ok(())
}

fn _sorted_provider_homes(agents_dir: &Utf8Path, provider: &str) -> Result<Vec<Utf8PathBuf>> {
    let mut homes = Vec::new();
    if !agents_dir.exists() {
        return Ok(homes);
    }
    for agent_entry in fs::read_dir(agents_dir)?.flatten() {
        let agent_path = agent_entry.path();
        let Ok(agent_path) = Utf8PathBuf::from_path_buf(agent_path) else {
            continue;
        };
        let home = agent_path
            .join("provider-state")
            .join(provider)
            .join("home");
        if home.exists() {
            homes.push(home);
        }
    }
    homes.sort();
    Ok(homes)
}

fn _cleanup_one_claude_versions_dir(
    versions_dir: &Utf8PathBuf,
    active_version_names: &HashSet<String>,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    if versions_dir.is_symlink() {
        skipped.push(CleanupSkipped {
            provider: "claude".into(),
            path: versions_dir.to_string(),
            reason: "versions_dir_is_symlink".into(),
        });
        return Ok(());
    }
    if !versions_dir.is_dir() {
        return Ok(());
    }
    let version_paths = _claude_version_paths(versions_dir)?;
    if version_paths.is_empty() {
        return Ok(());
    }
    if active_version_names.is_empty() {
        skipped.push(CleanupSkipped {
            provider: "claude".into(),
            path: versions_dir.to_string(),
            reason: "current_version_symlink_unresolved".into(),
        });
        return Ok(());
    }
    _prune_claude_versions(
        versions_dir,
        &version_paths,
        active_version_names,
        "claude",
        "old_claude_version_cache",
        actions,
        skipped,
    );
    Ok(())
}

fn _cleanup_shared_claude_versions_dir(
    versions_dir: &Utf8PathBuf,
    active_version_names: &HashSet<String>,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    if !versions_dir.exists() {
        return Ok(());
    }
    if versions_dir.is_symlink() {
        skipped.push(CleanupSkipped {
            provider: "claude".into(),
            path: versions_dir.to_string(),
            reason: "shared_versions_dir_is_symlink".into(),
        });
        return Ok(());
    }
    if !versions_dir.is_dir() {
        return Ok(());
    }
    let version_paths = _claude_version_paths(versions_dir)?;
    if version_paths.is_empty() {
        return Ok(());
    }
    let reason = if active_version_names.is_empty() {
        "unreferenced_shared_claude_version_cache"
    } else {
        "old_shared_claude_version_cache"
    };
    _prune_claude_versions(
        versions_dir,
        &version_paths,
        active_version_names,
        "claude",
        reason,
        actions,
        skipped,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn _prune_claude_versions(
    versions_dir: &Utf8PathBuf,
    version_paths: &[Utf8PathBuf],
    active_version_names: &HashSet<String>,
    provider: &str,
    reason: &str,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) {
    let keep = _claude_version_keep_paths(version_paths, active_version_names);
    for path in version_paths {
        if keep.contains(path) {
            continue;
        }
        _remove_tree(
            path,
            versions_dir,
            provider,
            "version_cache",
            reason,
            actions,
            skipped,
        );
    }
}

fn _claude_version_keep_paths(
    version_paths: &[Utf8PathBuf],
    active_version_names: &HashSet<String>,
) -> HashSet<Utf8PathBuf> {
    let mut keep: HashSet<Utf8PathBuf> = version_paths
        .iter()
        .filter(|p| {
            p.file_name()
                .map(|n| active_version_names.contains(n))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    if active_version_names.is_empty() {
        return keep;
    }

    let not_keep: Vec<_> = version_paths
        .iter()
        .filter(|p| !keep.contains(*p))
        .cloned()
        .collect();
    if let Some(rollback) = _newest_version_path(&not_keep) {
        keep.insert(rollback);
    }
    keep
}

fn _current_claude_version_name(home: &Utf8PathBuf) -> Option<String> {
    let link = home.join(".local/bin/claude");
    let target = match fs::canonicalize(&link) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let target = Utf8PathBuf::from_path_buf(target).ok()?;

    let versions_dir = home.join(".local/share/claude/versions");
    if !_is_within(&target, &versions_dir) {
        return None;
    }

    let resolved_versions = match fs::canonicalize(&versions_dir) {
        Ok(v) => Utf8PathBuf::from_path_buf(v).ok()?,
        Err(_) => return None,
    };

    let relative = target.strip_prefix(&resolved_versions).ok()?;
    let first = relative.iter().next()?;
    if first.is_empty() {
        return None;
    }
    Some(first.to_string())
}

fn _claude_version_paths(versions_dir: &Utf8PathBuf) -> Result<Vec<Utf8PathBuf>> {
    let entries = match fs::read_dir(versions_dir) {
        Ok(it) => it,
        Err(_) => return Ok(Vec::new()),
    };

    let mut paths: Vec<Utf8PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(path) = Utf8PathBuf::from_path_buf(path) else {
            continue;
        };
        let Some(name) = path.file_name() else {
            continue;
        };
        if !_looks_like_claude_version_name(name) {
            continue;
        }
        if path.is_symlink() {
            continue;
        }
        if !path.is_file() && !path.is_dir() {
            continue;
        }
        if !_is_within(&path, versions_dir) {
            continue;
        }
        paths.push(path);
    }

    paths.sort_by(|a, b| {
        let a_key = _version_key(a.file_name().unwrap_or(""));
        let b_key = _version_key(b.file_name().unwrap_or(""));
        let a_mtime = _safe_mtime(a);
        let b_mtime = _safe_mtime(b);
        let a_name = a.as_str();
        let b_name = b.as_str();
        (a_key, a_mtime, a_name).cmp(&(b_key, b_mtime, b_name))
    });

    Ok(paths)
}

fn _looks_like_claude_version_name(value: &str) -> bool {
    if value.is_empty() || !value.chars().next().unwrap_or('\0').is_ascii_digit() {
        return false;
    }
    value
        .chars()
        .all(|ch| ch.is_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
}

fn _newest_version_path(paths: &[Utf8PathBuf]) -> Option<Utf8PathBuf> {
    paths
        .iter()
        .max_by(|a, b| {
            let a_key = _version_key(a.file_name().unwrap_or(""));
            let b_key = _version_key(b.file_name().unwrap_or(""));
            let a_mtime = _safe_mtime(a);
            let b_mtime = _safe_mtime(b);
            let a_name = a.as_str();
            let b_name = b.as_str();
            (a_key, a_mtime, a_name).cmp(&(b_key, b_mtime, b_name))
        })
        .cloned()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum VersionPart {
    Text(String),
    Numeric(u64),
}

fn _version_key(value: &str) -> Vec<VersionPart> {
    value
        .replace('-', ".")
        .split('.')
        .map(|item| {
            if item.chars().all(|c| c.is_ascii_digit()) && !item.is_empty() {
                VersionPart::Numeric(item.parse().unwrap_or(0))
            } else {
                VersionPart::Text(item.to_string())
            }
        })
        .collect()
}

fn _safe_mtime(path: &Utf8Path) -> u64 {
    match fs::metadata(path) {
        Ok(meta) => match meta.modified() {
            Ok(t) => t
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            Err(_) => 0,
        },
        Err(_) => 0,
    }
}

fn _same_path(left: &Utf8Path, right: &Utf8Path) -> bool {
    let left_resolved = match fs::canonicalize(left) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let right_resolved = match fs::canonicalize(right) {
        Ok(p) => p,
        Err(_) => return false,
    };
    left_resolved == right_resolved
}

fn _cleanup_claude_rebuildable_caches(
    layout: &ccbr_storage::paths::PathLayout,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    let agents_dir = layout.agents_dir();
    if !agents_dir.exists() {
        return Ok(());
    }
    let safe_rels = _safe_claude_cache_rels();
    for home in _sorted_provider_homes(&agents_dir, "claude")? {
        if !home.is_dir() || home.is_symlink() {
            continue;
        }
        for relative in &safe_rels {
            let path = home.join(relative);
            if !path.exists() {
                continue;
            }
            _remove_tree(
                &path,
                &home,
                "claude",
                "tool_cache",
                "rebuildable_claude_cache",
                actions,
                skipped,
            );
        }
    }
    Ok(())
}

fn _safe_claude_cache_rels() -> Vec<Utf8PathBuf> {
    vec![
        Utf8PathBuf::from(".cache/claude"),
        Utf8PathBuf::from(".npm/_logs"),
        Utf8PathBuf::from(".claude/cache"),
        Utf8PathBuf::from(".claude/telemetry"),
        Utf8PathBuf::from(".claude/paste-cache"),
        Utf8PathBuf::from(".claude/plugins/marketplaces"),
    ]
}

fn _cleanup_gemini_rebuildable_caches(
    layout: &ccbr_storage::paths::PathLayout,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    let agents_dir = layout.agents_dir();
    if agents_dir.exists() {
        let safe_rels = _safe_gemini_cache_rels();
        for home in _sorted_provider_homes(&agents_dir, "gemini")? {
            if !home.is_dir() || home.is_symlink() {
                continue;
            }
            for relative in &safe_rels {
                let path = home.join(relative);
                if !path.exists() {
                    continue;
                }
                _remove_tree(
                    &path,
                    &home,
                    "gemini",
                    "tool_cache",
                    "rebuildable_gemini_cache",
                    actions,
                    skipped,
                );
            }
        }
    }

    let shared_gemini = layout.shared_cache_dir().join("gemini");
    _cleanup_gemini_cache_root(&shared_gemini, actions, skipped)?;

    let external_gemini = layout.provider_external_cache_dir("gemini")?;
    _cleanup_gemini_cache_root(&external_gemini, actions, skipped)?;

    Ok(())
}

fn _safe_gemini_cache_rels() -> Vec<Utf8PathBuf> {
    vec![
        Utf8PathBuf::from(".npm/_cacache"),
        Utf8PathBuf::from(".cache/node-gyp"),
        Utf8PathBuf::from(".cache/vscode-ripgrep"),
    ]
}

fn _gemini_shared_cache_rels() -> Vec<Utf8PathBuf> {
    vec![
        Utf8PathBuf::from("npm/_cacache"),
        Utf8PathBuf::from("xdg/node-gyp"),
        Utf8PathBuf::from("xdg/vscode-ripgrep"),
    ]
}

fn _cleanup_gemini_cache_root(
    cache_root: &Utf8PathBuf,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    if !cache_root.exists() || cache_root.is_symlink() {
        return Ok(());
    }
    for relative in _gemini_shared_cache_rels() {
        let path = cache_root.join(relative);
        if !path.exists() {
            continue;
        }
        _remove_tree(
            &path,
            cache_root,
            "gemini",
            "tool_cache",
            "rebuildable_gemini_cache",
            actions,
            skipped,
        );
    }
    Ok(())
}

fn _cleanup_pane_crash_logs(
    layout: &ccbr_storage::paths::PathLayout,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) -> Result<()> {
    let agents_dir = layout.agents_dir();
    if !agents_dir.exists() {
        return Ok(());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    for agent_entry in fs::read_dir(&agents_dir)?.flatten() {
        let agent_path = agent_entry.path();
        let Ok(agent_path) = Utf8PathBuf::from_path_buf(agent_path) else {
            continue;
        };
        let runtime_root = agent_path.join("provider-runtime");
        if !runtime_root.exists() {
            continue;
        }
        for runtime_entry in fs::read_dir(&runtime_root)?.flatten() {
            let runtime_dir = runtime_entry.path();
            let Ok(runtime_dir) = Utf8PathBuf::from_path_buf(runtime_dir) else {
                continue;
            };
            if !runtime_dir.is_dir() || runtime_dir.is_symlink() {
                continue;
            }
            let provider = runtime_dir.file_name().unwrap_or("unknown").to_string();

            let mut logs: Vec<Utf8PathBuf> = Vec::new();
            for log_entry in fs::read_dir(&runtime_dir)?.flatten() {
                let path = log_entry.path();
                let Ok(path) = Utf8PathBuf::from_path_buf(path) else {
                    continue;
                };
                let Some(name) = path.file_name() else {
                    continue;
                };
                if !name.starts_with("pane-crash-") || !name.ends_with(".log") {
                    continue;
                }
                if path.is_symlink() || !path.is_file() {
                    continue;
                }
                logs.push(path);
            }
            logs.sort_by(|a, b| {
                let a_mtime = _safe_mtime(a);
                let b_mtime = _safe_mtime(b);
                let a_name = a.as_str();
                let b_name = b.as_str();
                (a_mtime, a_name).cmp(&(b_mtime, b_name))
            });
            logs.reverse();

            for (index, path) in logs.iter().enumerate() {
                let age = now - _safe_mtime(path);
                if index < PANE_CRASH_LOG_MAX_KEEP_PER_RUNTIME
                    && age < PANE_CRASH_LOG_MAX_AGE_SECONDS
                {
                    continue;
                }
                _remove_tree(
                    path,
                    &runtime_dir,
                    &provider,
                    "crash_log",
                    "old_pane_crash_log",
                    actions,
                    skipped,
                );
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn _remove_tree(
    path: &Utf8Path,
    root: &Utf8Path,
    provider: &str,
    kind: &str,
    reason: &str,
    actions: &mut Vec<CleanupAction>,
    skipped: &mut Vec<CleanupSkipped>,
) {
    if path.is_symlink() {
        skipped.push(CleanupSkipped {
            provider: provider.into(),
            path: path.to_string(),
            reason: "symlink_not_removed".into(),
        });
        return;
    }
    if !_is_within(path, root) {
        skipped.push(CleanupSkipped {
            provider: provider.into(),
            path: path.to_string(),
            reason: "path_out_of_bounds".into(),
        });
        return;
    }

    let size = _tree_size(path);
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    if result.is_err() {
        return;
    }

    actions.push(CleanupAction {
        provider: provider.into(),
        kind: kind.into(),
        path: path.to_string(),
        bytes_removed: size,
        reason: reason.into(),
    });
}

fn _tree_size(path: &Utf8Path) -> u64 {
    if !path.exists() && !path.is_symlink() {
        return 0;
    }
    if path.is_file() || path.is_symlink() {
        return _lstat_size(path);
    }
    if !path.is_dir() {
        return 0;
    }

    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let Ok(child) = Utf8PathBuf::from_path_buf(child) else {
                continue;
            };
            total += _lstat_size(&child);
            if child.is_dir() && !child.is_symlink() {
                stack.push(child);
            }
        }
    }
    total
}

fn _lstat_size(path: &Utf8Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

fn _is_within(path: &Utf8Path, root: &Utf8Path) -> bool {
    let resolved_path = match fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let resolved_root = match fs::canonicalize(root) {
        Ok(p) => p,
        Err(_) => return false,
    };
    resolved_path.starts_with(&resolved_root)
}
