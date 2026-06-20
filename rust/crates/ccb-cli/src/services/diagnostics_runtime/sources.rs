//! Mirrors Python `lib/cli/services/diagnostics_runtime/sources.py`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::Value;

use ccb_storage::path_helpers::RootKind;

use crate::context::CliContext;

const _TAIL_SUFFIXES: &[&str] = &[".log", ".jsonl", ".txt", ".yaml", ".yml"];
const _COPY_SUFFIXES: &[&str] = &[".json", ".pid"];
const _PROVIDER_STATE_SUFFIXES: &[&str] = &[
    ".log",
    ".jsonl",
    ".txt",
    ".yaml",
    ".yml",
    ".json",
    ".pid",
    ".toml",
];

const _PROVIDER_STATE_SECRET_FILENAMES: &[&str] = &[
    ".credentials.json",
    ".env",
    "auth.json",
    "google_accounts.json",
    "oauth_creds.json",
];

const _PROVIDER_STATE_SECRET_DIRNAMES: &[&str] = &[".codeisland"];

const _EXCLUDED_PROVIDER_STORAGE_CLASSES: &[&str] = &[
    "secret",
    "rebuildable_cache",
    "startup_authority_bundle",
];

const _PROVIDER_STATE_HARD_EXCLUDED_SEGMENTS: &[&[&str]] = &[
    &[".tmp", "plugins"],
    &[".local", "share", "claude", "versions"],
    &[".npm", "_cacache"],
    &[".cache", "node-gyp"],
    &[".cache", "vscode-ripgrep"],
];

/// Collect every source file that should be staged into a diagnostic bundle.
pub fn project_root_sources(
    context: &CliContext,
    storage_payload: Option<&Value>,
) -> Vec<(String, PathBuf)> {
    let storage_records = storage_records_by_path(storage_payload);

    let mut items: Vec<(String, PathBuf)> = vec![
        ("project-config".into(), as_path(&context.paths.config_path())),
        (
            "ccbd-authority".into(),
            as_path(&context.paths.ccbd_lifecycle_path()),
        ),
        ("ccbd-authority".into(), as_path(&context.paths.ccbd_lease_path())),
        (
            "ccbd-authority".into(),
            as_path(&context.paths.ccbd_keeper_path()),
        ),
        (
            "ccbd-authority".into(),
            as_path(&context.paths.ccbd_shutdown_intent_path()),
        ),
        ("ccbd-authority".into(), as_path(&context.paths.ccbd_state_path())),
        (
            "ccbd-authority".into(),
            as_path(&context.paths.ccbd_start_policy_path()),
        ),
        (
            "ccbd-report".into(),
            as_path(&context.paths.ccbd_startup_report_path()),
        ),
        (
            "ccbd-report".into(),
            as_path(&context.paths.ccbd_shutdown_report_path()),
        ),
        (
            "ccbd-report".into(),
            as_path(&context.paths.ccbd_restore_report_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_submissions_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_messages_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_attempts_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_replies_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_dead_letters_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_supervision_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_lifecycle_log_path()),
        ),
        (
            "ccbd-events".into(),
            as_path(&context.paths.ccbd_tmux_cleanup_history_path()),
        ),
        (
            "ccbd-log".into(),
            as_path(&context.paths.ccbd_dir().join("ccbd.stdout.log")),
        ),
        (
            "ccbd-log".into(),
            as_path(&context.paths.ccbd_dir().join("ccbd.stderr.log")),
        ),
        (
            "ccbd-log".into(),
            as_path(&context.paths.ccbd_dir().join("keeper.stdout.log")),
        ),
        (
            "ccbd-log".into(),
            as_path(&context.paths.ccbd_dir().join("keeper.stderr.log")),
        ),
    ];

    if context.paths.runtime_state_placement().root_kind == RootKind::Relocated
        || as_path(&context.paths.runtime_root_ref_path()).exists()
        || as_path(&context.paths.runtime_root_marker_path()).exists()
    {
        items.extend(vec![
            (
                "runtime-root".into(),
                as_path(&context.paths.runtime_root_ref_path()),
            ),
            (
                "runtime-root".into(),
                as_path(&context.paths.runtime_root_marker_path()),
            ),
        ]);
    }

    items.extend(iter_dir_files(
        "ccbd-execution",
        &context.paths.ccbd_executions_dir(),
        &[".json"],
    ));
    items.extend(iter_dir_files(
        "ccbd-snapshot",
        &context.paths.ccbd_snapshots_dir(),
        &[".json"],
    ));
    items.extend(iter_dir_files(
        "ccbd-cursor",
        &context.paths.ccbd_cursors_dir(),
        &[".json"],
    ));
    items.extend(iter_dir_files(
        "ccbd-heartbeat",
        &context.paths.ccbd_heartbeats_dir(),
        &[".json"],
    ));
    items.extend(iter_dir_files(
        "ccbd-maintenance-heartbeat",
        &context.paths.ccbd_maintenance_heartbeat_dir(),
        &[".json", ".jsonl"],
    ));
    items.extend(iter_dir_files(
        "ccbd-text-artifact",
        &context.paths.ccbd_text_artifacts_dir(),
        &[".txt"],
    ));
    items.extend(iter_dir_files(
        "ccbd-health",
        &context.paths.ccbd_provider_health_dir(),
        &[".jsonl"],
    ));
    items.extend(iter_dir_files(
        "ccbd-mailbox",
        &context.paths.ccbd_mailboxes_dir(),
        &[".json", ".jsonl"],
    ));
    items.extend(iter_dir_files(
        "ccbd-lease",
        &context.paths.ccbd_leases_dir(),
        &[".json"],
    ));

    items.extend(agent_source_items(context, &storage_records));
    items
}

/// Recursively collect files under `root` matching `suffixes`.
pub fn iter_dir_files(category: &str, root: &Utf8PathBuf, suffixes: &[&str]) -> Vec<(String, PathBuf)> {
    let root = as_path(root);
    if !root.exists() || !root.is_dir() {
        return vec![];
    }
    let mut files = vec![];
    for path in walk_sorted_files(&root) {
        if !path.is_file() {
            continue;
        }
        if path_has_suffix(&path, suffixes) {
            files.push((category.into(), path));
        }
    }
    files
}

/// Recursively collect provider-state files, excluding secrets, caches, and symlinks.
pub fn iter_provider_state_files(
    category: &str,
    root: &Utf8PathBuf,
    storage_records: &HashMap<String, Value>,
) -> Vec<(String, PathBuf)> {
    let root = as_path(root);
    if !root.exists() || !root.is_dir() {
        return vec![];
    }
    let mut files = vec![];
    for path in walk_provider_state_files(&root) {
        if !path_has_suffix(&path, _PROVIDER_STATE_SUFFIXES) {
            continue;
        }
        if provider_state_path_hard_excluded(&path, &root) {
            continue;
        }
        if let Some(class) = storage_class_for_path(&path, storage_records) {
            if _EXCLUDED_PROVIDER_STORAGE_CLASSES.contains(&class.as_str()) {
                continue;
            }
        }
        if path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .any(|part| {
                _PROVIDER_STATE_SECRET_DIRNAMES
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(part))
            })
        {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                _PROVIDER_STATE_SECRET_FILENAMES
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(n))
            })
            .unwrap_or(false)
        {
            continue;
        }
        files.push((category.into(), path));
    }
    files
}

fn walk_sorted_files(root: &Path) -> Vec<PathBuf> {
    let mut results = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries = match fs::read_dir(&dir) {
            Ok(e) => e.filter_map(|e| e.ok()).collect::<Vec<_>>(),
            Err(_) => continue,
        };
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if metadata.is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

fn walk_provider_state_files(root: &Path) -> Vec<PathBuf> {
    let mut results = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries = match fs::read_dir(&dir) {
            Ok(e) => e.filter_map(|e| e.ok()).collect::<Vec<_>>(),
            Err(_) => continue,
        };
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if metadata.is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if provider_state_path_hard_excluded(&path, root) {
                    continue;
                }
                stack.push(path);
            } else if metadata.is_file() {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

fn provider_state_path_hard_excluded(path: &Path, root: &Path) -> bool {
    let parts: Vec<String> = match path.strip_prefix(root) {
        Ok(rel) => rel
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_lowercase()))
            .collect(),
        Err(_) => path
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_lowercase()))
            .collect(),
    };
    if parts.len() >= 2 && parts[parts.len() - 2..] == [".tmp".to_string(), "plugins.sha".to_string()] {
        return true;
    }
    _PROVIDER_STATE_HARD_EXCLUDED_SEGMENTS
        .iter()
        .any(|segment| parts_contain(&parts, segment))
}

fn parts_contain(parts: &[String], segment: &[&str]) -> bool {
    if segment.is_empty() || parts.len() < segment.len() {
        return false;
    }
    let limit = parts.len() - segment.len() + 1;
    (0..limit).any(|i| {
        segment
            .iter()
            .enumerate()
            .all(|(j, s)| parts[i + j].eq_ignore_ascii_case(s))
    })
}

pub fn iter_agent_dirs(context: &CliContext) -> Vec<PathBuf> {
    let root = as_path(&context.paths.agents_dir());
    if !root.exists() || !root.is_dir() {
        return vec![];
    }
    let mut dirs: Vec<PathBuf> = match fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect(),
        Err(_) => vec![],
    };
    dirs.sort();
    dirs
}

/// Extract a session path referenced by an agent runtime file, if any.
pub fn session_path_from_runtime(runtime_path: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(runtime_path).ok()?;
    let payload: Value = serde_json::from_str(&contents).ok()?;
    let payload = payload.as_object()?;
    for key in &["session_file", "session_ref"] {
        let candidate = payload.get(*key)?.as_str()?;
        if candidate.trim().is_empty() {
            continue;
        }
        let path = expand_user_path(candidate);
        let path = PathBuf::from(path);
        if path.is_absolute() && path.exists() {
            return Some(path);
        }
    }
    None
}

/// Map a source path to its location inside the tarball.
pub fn archive_path_for_source(context: &CliContext, source: &Path) -> String {
    let source_path = resolve_source(source);
    let project_root = as_path(&context.paths.project_root);
    if let Ok(rel) = source_path.strip_prefix(&project_root) {
        return PathBuf::from("project").join(rel).to_string_lossy().to_string();
    }
    let runtime_root = as_path(context.paths.runtime_state_root());
    if let Ok(rel) = source_path.strip_prefix(&runtime_root) {
        return PathBuf::from("project")
            .join(".ccb")
            .join(rel)
            .to_string_lossy()
            .to_string();
    }
    let safe_parts: Vec<Component<'_>> = source
        .components()
        .filter(|c| !matches!(c, Component::RootDir | Component::Prefix(_)))
        .collect();
    let suffix: PathBuf = if safe_parts.len() >= 4 {
        safe_parts[safe_parts.len() - 4..]
            .iter()
            .collect()
    } else if let Some(name) = source.file_name() {
        PathBuf::from(name)
    } else {
        PathBuf::new()
    };
    PathBuf::from("external")
        .join(suffix)
        .to_string_lossy()
        .to_string()
}

fn agent_source_items(
    context: &CliContext,
    storage_records: &HashMap<String, Value>,
) -> Vec<(String, PathBuf)> {
    let mut items = vec![];
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for agent_dir in iter_agent_dirs(context) {
        let agent_name = agent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        for (category, path) in agent_sources(context, &agent_name, &agent_dir, storage_records) {
            let resolved = resolve_source(&path);
            if seen.contains(&resolved) {
                continue;
            }
            seen.insert(resolved);
            items.push((category, path));
        }
    }
    items
}

fn resolve_source(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
}

fn agent_sources(
    context: &CliContext,
    agent_name: &str,
    agent_dir: &Path,
    storage_records: &HashMap<String, Value>,
) -> Vec<(String, PathBuf)> {
    let mut items: Vec<(String, PathBuf)> = vec![
        (
            "agent-authority".into(),
            as_path(&context.paths.agent_spec_path(agent_name)),
        ),
        (
            "agent-authority".into(),
            as_path(&context.paths.agent_runtime_path(agent_name)),
        ),
        (
            "agent-authority".into(),
            as_path(&context.paths.agent_restore_path(agent_name)),
        ),
        (
            "agent-events".into(),
            as_path(&context.paths.agent_jobs_path(agent_name)),
        ),
        (
            "agent-events".into(),
            as_path(&context.paths.agent_events_path(agent_name)),
        ),
        (
            "agent-workspace".into(),
            as_path(&context.paths.workspace_binding_path(agent_name, None)),
        ),
    ];

    items.extend(iter_dir_files(
        "agent-log",
        &context.paths.agent_logs_dir(agent_name),
        _TAIL_SUFFIXES,
    ));
    items.extend(iter_dir_files(
        "agent-runtime",
        &Utf8PathBuf::from_path_buf(agent_dir.join("provider-runtime")).unwrap_or_default(),
        &[_TAIL_SUFFIXES, _COPY_SUFFIXES].concat(),
    ));
    items.extend(iter_provider_state_files(
        "agent-provider-state",
        &Utf8PathBuf::from_path_buf(agent_dir.join("provider-state")).unwrap_or_default(),
        storage_records,
    ));

    let runtime_path = as_path(&context.paths.agent_runtime_path(agent_name));
    if runtime_path.exists() {
        if let Some(session_path) = session_path_from_runtime(&runtime_path) {
            items.push(("agent-session".into(), session_path));
        }
    }
    items
}

fn storage_records_by_path(storage_payload: Option<&Value>) -> HashMap<String, Value> {
    let mut records: HashMap<String, Value> = HashMap::new();
    let entries = storage_payload
        .and_then(|p| p.get("entries"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for raw_entry in entries {
        let entry = match raw_entry.as_object() {
            Some(o) => o,
            None => continue,
        };
        let raw_path = match entry.get("path").and_then(Value::as_str) {
            Some(p) => p,
            None => continue,
        };
        if raw_path.is_empty() {
            continue;
        }
        let path = resolve_source(&PathBuf::from(raw_path));
        records.insert(path.to_string_lossy().to_string(), Value::Object(entry.clone()));
    }
    records
}

fn storage_class_for_path(path: &Path, storage_records: &HashMap<String, Value>) -> Option<String> {
    let resolved = resolve_source(path);
    let record = storage_records.get(resolved.to_string_lossy().as_ref())?;
    let class = record
        .get("storage_class")
        .and_then(Value::as_str)?
        .trim()
        .to_lowercase();
    if class.is_empty() {
        return None;
    }
    Some(class)
}

fn as_path(utf8: &Utf8Path) -> PathBuf {
    utf8.as_std_path().to_path_buf()
}

fn path_has_suffix(path: &Path, suffixes: &[&str]) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    suffixes
        .iter()
        .any(|s| s.strip_prefix('.').unwrap_or(s).eq_ignore_ascii_case(ext))
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}
