use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::error::{ProviderCoreError, Result};

const HASH_CHUNK_SIZE: usize = 64 * 1024;

/// Marker stored alongside a projected tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionMarker {
    pub schema_version: i32,
    pub record_type: String,
    pub label: String,
    pub source: String,
    pub mode: String,
    pub updated_at: String,
}

/// Route a source directory to a target path as a symlink or copy.
///
/// Returns `true` if the target exists and points at the source afterwards.
#[allow(clippy::too_many_arguments)]
pub fn route_projected_tree(
    source: &Path,
    target: &Path,
    enabled: bool,
    label: &str,
    marker_path: Option<&Path>,
    allow_unmarked_replace: bool,
) -> Result<bool> {
    let source = expand_home(source);
    let target = expand_home(target);
    let marker = marker_path
        .map(expand_home)
        .unwrap_or_else(|| default_marker_path(&target));

    if !enabled || !source.is_dir() {
        remove_projected_target(&target, &marker, allow_unmarked_replace)?;
        return Ok(false);
    }
    if same_path(&source, &target) {
        return Ok(true);
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if projection_points_to(&target, &source) {
        write_projection_marker(&marker, &source, "symlink", label)?;
        return Ok(true);
    }

    if target.exists() || target.is_symlink() {
        if !can_replace_projected_target(&target, &marker, allow_unmarked_replace, Some(&source))? {
            return Ok(false);
        }
        remove_path(&target);
    }

    if try_symlink_or_copy(&source, &target, label, &marker)? {
        return Ok(true);
    }
    Ok(false)
}

fn try_symlink_or_copy(source: &Path, target: &Path, label: &str, marker: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(source, target).is_ok() {
            write_projection_marker(marker, source, "symlink", label)?;
            return Ok(true);
        }
        tracing::debug!("symlink failed, falling back to copy");
    }
    #[cfg(not(unix))]
    {
        tracing::debug!("symlinks not supported on this platform, using copy");
    }
    remove_path(target);
    match copy_tree(source, target) {
        Ok(()) => {
            write_projection_marker(marker, source, "copy", label)?;
            Ok(true)
        }
        Err(e) => {
            remove_path(target);
            Err(e)
        }
    }
}

/// Copy a source directory into a cache/bundle root if it has the required
/// entries, otherwise replace the bundle with a full copy.
pub fn copy_projected_tree_to_cache(
    source: &Path,
    bundle_root: &Path,
    label: &str,
) -> Result<bool> {
    let source = expand_home(source);
    let bundle_root = expand_home(bundle_root);
    if !source.is_dir() {
        return Ok(false);
    }
    if tree_has_required_entries(&source, &bundle_root) {
        write_projected_marker(&bundle_root, label, "copy", &source)?;
        return Ok(true);
    }
    let tmp_root = bundle_root.with_file_name(format!(
        ".{}.tmp",
        bundle_root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    remove_path(&tmp_root);
    if let Some(parent) = tmp_root.parent() {
        fs::create_dir_all(parent)?;
    }
    match copy_tree(&source, &tmp_root) {
        Ok(()) => {
            remove_path(&bundle_root);
            match fs::rename(&tmp_root, &bundle_root) {
                Ok(()) => {
                    write_projected_marker(&bundle_root, label, "copy", &source)?;
                    Ok(true)
                }
                Err(e) => {
                    remove_path(&tmp_root);
                    Err(e.into())
                }
            }
        }
        Err(e) => {
            remove_path(&tmp_root);
            Err(e)
        }
    }
}

/// Ensure a shared tree bundle exists for a source directory.
pub fn ensure_shared_tree_bundle(source: &Path, bundle_root: &Path) -> Option<PathBuf> {
    if copy_projected_tree_to_cache(source, bundle_root, "projected-tree").ok()? {
        Some(bundle_root.to_path_buf())
    } else {
        None
    }
}

/// Remove a projected path if its marker matches.
pub fn remove_projected_path(
    target: &Path,
    label: &str,
    source: Option<&Path>,
    marker_path: Option<&Path>,
    allow_unmarked_replace: bool,
) -> Result<()> {
    let target = expand_home(target);
    let marker = marker_path
        .map(expand_home)
        .unwrap_or_else(|| default_marker_path(&target));
    if marker_matches(&marker, label, source) {
        remove_projected_target(&target, &marker, allow_unmarked_replace)?;
    } else if allow_unmarked_replace && target.is_symlink() {
        remove_path(&target);
    }
    Ok(())
}

/// Remove a projected tree if its marker matches.
pub fn remove_projected_tree(
    target: &Path,
    marker_path: Option<&Path>,
    allow_unmarked_replace: bool,
) -> Result<()> {
    remove_projected_path(
        target,
        "projected-tree",
        None,
        marker_path,
        allow_unmarked_replace,
    )
}

/// Write a projection marker for a target path.
pub fn write_projected_marker(target: &Path, label: &str, mode: &str, source: &Path) -> Result<()> {
    let marker = default_marker_path(&expand_home(target));
    write_projection_marker(&marker, &expand_home(source), mode, label)
}

/// Compute a content fingerprint for a directory tree.
pub fn tree_content_fingerprint(root: &Path) -> String {
    let root = expand_home(root);
    let mut hasher = sha2::Sha256::new();
    let mut entries: Vec<PathBuf> = match walk_dir(&root) {
        Ok(e) => e,
        Err(_) => return String::new(),
    };
    entries.sort();
    for entry in entries {
        let relative = match entry.strip_prefix(&root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let kind = if entry.is_dir() {
            'd'
        } else if entry.is_file() {
            'f'
        } else if entry.is_symlink() {
            'l'
        } else {
            'o'
        };
        hasher.update(kind.to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        if entry.is_file() {
            if hash_file(&entry, &mut hasher).is_err() {
                return String::new();
            }
        } else if entry.is_symlink() {
            if let Ok(target) = fs::read_link(&entry) {
                hasher.update(target.to_string_lossy().as_bytes());
            }
        }
        hasher.update(b"\0");
    }
    hex::encode(hasher.finalize())
}

fn hash_file(path: &Path, hasher: &mut sha2::Sha256) -> std::io::Result<()> {
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; HASH_CHUNK_SIZE];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(())
}

fn walk_dir(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    if root.is_dir() {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            result.push(path.clone());
            if path.is_dir() {
                result.extend(walk_dir(&path)?);
            }
        }
    }
    Ok(result)
}

fn remove_projected_target(
    target: &Path,
    marker: &Path,
    allow_unmarked_replace: bool,
) -> Result<()> {
    if can_replace_projected_target(target, marker, allow_unmarked_replace, None)? {
        remove_path(target);
        let _ = fs::remove_file(marker);
    }
    Ok(())
}

fn projection_points_to(target: &Path, source: &Path) -> bool {
    if !target.is_symlink() {
        return false;
    }
    match target.canonicalize() {
        Ok(resolved) => match source.canonicalize() {
            Ok(src) => resolved == src,
            Err(_) => false,
        },
        Err(_) => match fs::read_link(target) {
            Ok(link) => link == source,
            Err(_) => false,
        },
    }
}

fn can_replace_projected_target(
    target: &Path,
    marker: &Path,
    allow_unmarked_replace: bool,
    replacement_source: Option<&Path>,
) -> Result<bool> {
    if marker.is_file() {
        return Ok(true);
    }
    if !target.exists() {
        return Ok(true);
    }
    if target.is_symlink() {
        return Ok(allow_unmarked_replace);
    }
    if allow_unmarked_replace {
        return Ok(true);
    }
    if let Some(src) = replacement_source {
        if target.is_dir() && src.is_dir() {
            return Ok(tree_content_fingerprint(target) == tree_content_fingerprint(src));
        }
    }
    Ok(allow_unmarked_replace)
}

fn tree_has_required_entries(source: &Path, candidate: &Path) -> bool {
    if !candidate.is_dir() {
        return false;
    }
    let entries = match walk_dir(source) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries {
        let relative = match entry.strip_prefix(source) {
            Ok(r) => r,
            Err(_) => return false,
        };
        let projected = candidate.join(relative);
        if entry.is_dir() && !projected.is_dir() {
            return false;
        }
        if entry.is_file() && !projected.is_file() {
            return false;
        }
        if entry.is_symlink() && !projected.exists() && !projected.is_symlink() {
            return false;
        }
    }
    true
}

fn default_marker_path(target: &Path) -> PathBuf {
    PathBuf::from(format!("{}.ccbr-projection.json", target.display()))
}

fn marker_matches(marker: &Path, label: &str, source: Option<&Path>) -> bool {
    let text = match fs::read_to_string(marker) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let payload: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let obj = match payload.as_object() {
        Some(o) => o,
        None => return false,
    };
    if obj.get("record_type").and_then(|v| v.as_str()) != Some("ccbr_projected_asset") {
        return false;
    }
    if obj.get("label").and_then(|v| v.as_str()).unwrap_or("") != label {
        return false;
    }
    if let Some(src) = source {
        let marker_src = obj.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let a = expand_home(&PathBuf::from(marker_src));
        let b = expand_home(src);
        if let (Ok(aa), Ok(bb)) = (a.canonicalize(), b.canonicalize()) {
            return aa == bb;
        }
        return a == b;
    }
    true
}

fn write_projection_marker(path: &Path, source: &Path, mode: &str, label: &str) -> Result<()> {
    let marker = ProjectionMarker {
        schema_version: 1,
        record_type: "ccbr_projected_asset".to_string(),
        label: label.to_string(),
        source: source.to_string_lossy().to_string(),
        mode: mode.to_string(),
        updated_at: chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            .replace("+00:00", "Z"),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(&marker)?;
    fs::write(path, json + "\n")?;
    Ok(())
}

fn remove_path(path: &Path) {
    if path.is_symlink() || path.is_file() {
        let _ = fs::remove_file(path);
        return;
    }
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => left == right,
    }
}

fn expand_home(path: &Path) -> PathBuf {
    if let Some(std::path::Component::Normal(seg)) = path.components().next() {
        if seg == "~" {
            if let Ok(home) = std::env::var("HOME") {
                let rest: PathBuf = path.components().skip(1).collect();
                return PathBuf::from(home).join(rest);
            }
        }
    }
    path.to_path_buf()
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        return Err(ProviderCoreError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "destination already exists",
        )));
    }
    fs::create_dir_all(dst)?;
    for entry in walk_dir(src)? {
        let relative = entry.strip_prefix(src).map_err(|_| {
            ProviderCoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "entry not under source",
            ))
        })?;
        let target = dst.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&entry, &target)?;
        } else if entry.is_symlink() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if let Ok(link_target) = fs::read_link(&entry) {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&link_target, &target)?;
                #[cfg(not(unix))]
                {
                    let _ = link_target;
                    return Err(ProviderCoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "symlinks not supported on this platform",
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_content_fingerprint_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(!tree_content_fingerprint(tmp.path()).is_empty());
    }
}
