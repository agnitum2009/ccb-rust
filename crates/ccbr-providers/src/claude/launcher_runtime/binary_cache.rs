//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/binary_cache.py`.
//!
//! Routes the Claude per-home `versions` cache into a shared external cache
//! directory.

use std::io;

use camino::Utf8Path;

/// Route the Claude `versions` cache from an isolated home into a shared
/// external cache directory.
///
/// Mirrors Python `route_claude_binary_cache`.
pub fn route_claude_binary_cache(
    home_root: &Utf8Path,
    cache_root: &Utf8Path,
    source_home: Option<&Utf8Path>,
) -> io::Result<()> {
    let _ = source_home;
    let versions_dir = home_root
        .join(".local")
        .join("share")
        .join("claude")
        .join("versions");
    let cache_versions = cache_root.join("versions");

    // If the versions directory already points to the shared cache, nothing to do.
    if versions_dir.is_symlink() {
        if let Ok(target) = std::fs::read_link(&versions_dir) {
            if target == cache_versions.as_std_path() {
                return Ok(());
            }
        }
    }

    std::fs::create_dir_all(&cache_versions)?;

    // If there is an existing real versions directory, move its contents into
    // the shared cache.
    if versions_dir.is_dir() && !versions_dir.is_symlink() {
        for entry in std::fs::read_dir(&versions_dir)? {
            let entry = entry?;
            let dest = cache_versions.join(entry.file_name().to_string_lossy().as_ref());
            std::fs::rename(entry.path(), dest)?;
        }
        std::fs::remove_dir(&versions_dir)?;
    }

    if let Some(parent) = versions_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        if !versions_dir.exists() {
            std::os::unix::fs::symlink(&cache_versions, &versions_dir)?;
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix platforms, copy the cache tree back rather than symlinking.
        copy_dir_all(&cache_versions, &versions_dir)?;
    }

    Ok(())
}

#[cfg(not(unix))]
fn copy_dir_all(src: &Utf8Path, dst: &Utf8Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = Utf8PathBuf::from_path_buf(entry.path())
            .unwrap_or_else(|_| src.join(entry.file_name().to_string_lossy()));
        let dst_path = dst.join(entry.file_name().to_string_lossy());
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
