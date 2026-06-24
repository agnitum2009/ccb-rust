use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::Builder;
use walkdir::WalkDir;

pub const DEFAULT_OUTPUT_DIR: &str = "dist";

pub const EXCLUDES: &[&str] = &[
    ".git",
    ".ccbr",
    ".ccbr-requests",
    ".architec",
    ".claude",
    ".codex",
    ".codegraph",
    ".gemini",
    ".hippocampus",
    ".loop",
    ".tmp_pytest",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".venv",
    "target",
    "lib",
    "test",
    "ccbr_test",
    "dev_tools",
    "dist",
    "roles",
];

/// Rust binaries that must be built, packaged, and installed from the release artifact.
pub const REQUIRED_BINARIES: &[&str] = &["ccbr", "ccbrd", "ask", "autonew", "ctx-transfer"];

const HOST_SYSTEMS: &[(&str, &str)] = &[("linux", "Linux"), ("macos", "Darwin")];

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub platform: String,
    pub output_dir: PathBuf,
    pub channel: Option<String>,
    pub git_ref: String,
    pub allow_dirty: bool,
}

#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub artifact_path: PathBuf,
    pub sha_path: PathBuf,
    pub version: String,
    pub commit: Option<String>,
    pub channel: String,
    pub platform: String,
}

#[derive(Debug, Clone)]
pub struct PackageOptions {
    pub platform: String,
    pub artifact_dir: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PackageOutput {
    pub artifact_path: PathBuf,
    pub sha_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct VerifyOptions {
    pub artifact_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct BuildInfo {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub build_time: String,
    pub platform: String,
    pub arch: String,
    pub channel: String,
    pub source_kind: String,
    pub install_mode: String,
}

pub fn run_build(opts: &BuildOptions) -> Result<BuildOutput> {
    let repo_root = find_repo_root()?;
    let host_system = expected_host_system(&opts.platform)?;
    let current_system = current_system()?;
    let current_platform = normalize_release_platform(&current_system);
    let expected_platform = normalize_release_platform(&host_system);
    if current_platform != expected_platform {
        bail!(
            "release build for {} must run on {} (current host is {})",
            opts.platform,
            host_system,
            current_system
        );
    }

    fs::create_dir_all(&opts.output_dir)
        .with_context(|| format!("creating output dir {:?}", opts.output_dir))?;

    let channel = opts.channel.clone().unwrap_or_else(|| {
        if opts.allow_dirty {
            "preview".into()
        } else {
            "stable".into()
        }
    });

    let use_git_ref_source = is_git_checkout(&repo_root) && !opts.allow_dirty;
    let version = resolve_version(&repo_root)?;
    let (commit, commit_date) = resolve_git_metadata(
        &repo_root,
        if use_git_ref_source {
            Some(&opts.git_ref)
        } else {
            None
        },
    )?;

    let machine = current_machine()?;
    let artifact_basename =
        release_artifact_basename(&opts.platform, &machine).ok_or_else(|| {
            anyhow!(
                "unsupported release target platform={} machine={}",
                opts.platform,
                machine
            )
        })?;

    let stage_root = opts
        .output_dir
        .join(format!(".stage-{}", artifact_basename));
    let artifact_root = stage_root.join(&artifact_basename);
    let artifact_path = opts
        .output_dir
        .join(format!("{}.tar.gz", artifact_basename));
    let sha_path = opts.output_dir.join("SHA256SUMS");

    if stage_root.exists() {
        fs::remove_dir_all(&stage_root)
            .with_context(|| format!("removing old stage root {:?}", stage_root))?;
    }
    if artifact_path.exists() {
        fs::remove_file(&artifact_path)
            .with_context(|| format!("removing old artifact {:?}", artifact_path))?;
    }

    let generated_paths: Vec<PathBuf> = vec![
        opts.output_dir.clone(),
        stage_root.clone(),
        artifact_path.clone(),
        sha_path.clone(),
    ];

    export_release_tree(
        &repo_root,
        &artifact_root,
        &opts.git_ref,
        opts.allow_dirty,
        &generated_paths,
    )?;

    build_rust_workspace_for_release(&artifact_root, &opts.platform)?;
    build_sidebar_helper_for_release(&artifact_root, &opts.platform)?;

    let arch = release_build_arch(&opts.platform, &machine).ok_or_else(|| {
        anyhow!(
            "unable to determine release arch for platform={}",
            opts.platform
        )
    })?;

    let build_info = BuildInfo {
        version: version.clone(),
        commit: commit.clone(),
        date: commit_date.clone(),
        build_time: utc_now(),
        platform: opts.platform.clone(),
        arch,
        channel: channel.clone(),
        source_kind: if opts.allow_dirty {
            "preview".into()
        } else {
            "release".into()
        },
        install_mode: "release".into(),
    };
    write_release_metadata(&artifact_root, &build_info)?;

    create_tarball(&stage_root, &artifact_root, &artifact_path)?;
    write_sha256(&artifact_path, &sha_path)?;

    println!("artifact: {}", artifact_path.display());
    println!("sha256: {}", sha_path.display());
    println!("version: {}", version);
    println!("commit: {}", commit.as_deref().unwrap_or(""));
    println!("channel: {}", channel);
    println!("platform: {}", opts.platform);
    if opts.allow_dirty {
        println!("warning: built from current dirty worktree for local preview only");
    }

    Ok(BuildOutput {
        artifact_path,
        sha_path,
        version,
        commit,
        channel,
        platform: opts.platform.clone(),
    })
}

pub fn run_package(opts: &PackageOptions) -> Result<PackageOutput> {
    fs::create_dir_all(&opts.output_dir)
        .with_context(|| format!("creating output dir {:?}", opts.output_dir))?;

    if !opts.artifact_dir.is_dir() {
        bail!("artifact directory does not exist: {:?}", opts.artifact_dir);
    }

    let machine = current_machine()?;
    let artifact_basename =
        release_artifact_basename(&opts.platform, &machine).ok_or_else(|| {
            anyhow!(
                "unsupported release target platform={} machine={}",
                opts.platform,
                machine
            )
        })?;

    let stage_root = opts
        .output_dir
        .join(format!(".stage-{}", artifact_basename));
    let artifact_root = stage_root.join(&artifact_basename);
    let artifact_path = opts
        .output_dir
        .join(format!("{}.tar.gz", artifact_basename));
    let sha_path = opts.output_dir.join("SHA256SUMS");

    if stage_root.exists() {
        fs::remove_dir_all(&stage_root)?;
    }
    if artifact_path.exists() {
        fs::remove_file(&artifact_path)?;
    }

    copy_dir_contents(&opts.artifact_dir, &artifact_root)?;
    create_tarball(&stage_root, &artifact_root, &artifact_path)?;
    write_sha256(&artifact_path, &sha_path)?;

    Ok(PackageOutput {
        artifact_path,
        sha_path,
    })
}

pub fn run_verify(opts: &VerifyOptions) -> Result<()> {
    if !opts.artifact_path.is_file() {
        bail!("artifact not found: {:?}", opts.artifact_path);
    }

    let file = fs::File::open(&opts.artifact_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let mut found_build_info = false;
    let mut found_binaries: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let path = entry.path()?;
        let path_str = path.as_os_str().to_string_lossy();
        if path_str.ends_with("BUILD_INFO.json") {
            found_build_info = true;
        }
        for name in REQUIRED_BINARIES {
            if path_str.ends_with(&format!("/bin/{}", name)) || path_str.ends_with(name) {
                found_binaries.insert(*name);
            }
        }
    }

    if !found_build_info {
        bail!("artifact missing BUILD_INFO.json");
    }

    let missing: Vec<&str> = REQUIRED_BINARIES
        .iter()
        .copied()
        .filter(|name| !found_binaries.contains(name))
        .collect();
    if !missing.is_empty() {
        bail!("artifact missing required binaries: {:?}", missing);
    }

    Ok(())
}

fn find_repo_root() -> Result<PathBuf> {
    let start = std::env::current_dir()?;
    let mut current: Option<&Path> = Some(&start);
    while let Some(dir) = current {
        if dir.join("rust").join("Cargo.toml").exists() && dir.join("VERSION").exists() {
            return Ok(dir.to_path_buf());
        }
        current = dir.parent();
    }

    // Fallback: ask git for the top-level directory.
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&start)
        .output()
        .context("running git rev-parse --show-toplevel")?;
    if output.status.success() {
        let root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if root.join("rust").join("Cargo.toml").exists() {
            return Ok(root);
        }
    }

    bail!("unable to locate CCBR repository root (looked for rust/Cargo.toml + VERSION)")
}

fn current_system() -> Result<String> {
    Ok(std::env::consts::OS.to_string())
}

fn current_machine() -> Result<String> {
    Ok(std::env::consts::ARCH.to_string())
}

fn expected_host_system(target_platform: &str) -> Result<String> {
    let target_platform = target_platform.trim();
    for (platform, system) in HOST_SYSTEMS {
        if *platform == target_platform {
            return Ok((*system).into());
        }
    }
    bail!("unsupported release target platform: {}", target_platform)
}

pub fn resolve_version(repo_root: &Path) -> Result<String> {
    let version_file = repo_root.join("VERSION");
    if version_file.is_file() {
        let value = fs::read_to_string(&version_file)
            .with_context(|| format!("reading {:?}", version_file))?
            .trim()
            .to_string();
        if !value.is_empty() {
            return Ok(value);
        }
    }
    bail!("unable to resolve version from VERSION")
}

pub fn resolve_git_metadata(
    repo_root: &Path,
    git_ref: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    if !is_git_checkout(repo_root) {
        return Ok((None, None));
    }
    let resolved_ref = git_ref.unwrap_or("HEAD");
    let commit = run_git(repo_root, &["log", "-1", "--format=%h", resolved_ref])?;
    let commit_date = run_git(repo_root, &["log", "-1", "--format=%cs", resolved_ref])?;
    Ok((Some(commit), Some(commit_date)))
}

fn run_git(repo_root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .with_context(|| format!("running git {:?}", args))?;
    if !output.status.success() {
        bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn is_git_checkout(repo_root: &Path) -> bool {
    repo_root.join(".git").exists()
}

pub fn ensure_clean_worktree(repo_root: &Path) -> Result<()> {
    let entries = dirty_worktree_entries(repo_root)?;
    if entries.is_empty() {
        return Ok(());
    }
    let mut preview: Vec<String> = entries
        .iter()
        .take(20)
        .map(|e| format!("  {}", e))
        .collect();
    let remaining = entries.len().saturating_sub(20);
    if remaining > 0 {
        preview.push(format!("  ... and {} more", remaining));
    }
    bail!(
        "refusing to build release from a dirty worktree.\n\
         Commit or stash changes first, or pass --allow-dirty for a local preview build.\n\
         Dirty entries:\n{}",
        preview.join("\n")
    );
}

pub fn dirty_worktree_entries(repo_root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .context("running git status")?;
    if !output.status.success() {
        bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty() && !is_excluded_status_entry(line))
        .map(|line| line.trim_end().to_string())
        .collect())
}

fn is_excluded_status_entry(line: &str) -> bool {
    let text = line.trim_end();
    if text.len() < 4 {
        return false;
    }
    let payload = text[3..].trim();
    if payload.is_empty() {
        return false;
    }
    payload
        .split("->")
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .all(is_excluded_relpath)
}

pub fn is_excluded_part(part: &str) -> bool {
    let text = part.trim();
    if text.is_empty() {
        return false;
    }
    if EXCLUDES.contains(&text) {
        return true;
    }
    text.starts_with(".tmp_test_env_")
}

pub fn export_release_tree(
    repo_root: &Path,
    destination: &Path,
    git_ref: &str,
    allow_dirty: bool,
    generated_paths: &[PathBuf],
) -> Result<()> {
    if is_git_checkout(repo_root) {
        if allow_dirty {
            copy_repo_tree(repo_root, destination, generated_paths)?;
            return Ok(());
        }
        ensure_clean_worktree(repo_root)?;
        export_git_archive(repo_root, destination, git_ref)?;
        return Ok(());
    }
    copy_repo_tree(repo_root, destination, generated_paths)?;
    Ok(())
}

fn generated_relpaths_under_repo(repo_root: &Path, paths: &[PathBuf]) -> Vec<PathBuf> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut relpaths: Vec<PathBuf> = paths
        .iter()
        .filter_map(|raw| {
            let resolved = raw.canonicalize().unwrap_or_else(|_| raw.to_path_buf());
            resolved
                .strip_prefix(&repo_root)
                .ok()
                .map(|p| p.to_path_buf())
        })
        .collect();
    relpaths.sort_by_key(|p| (p.components().count(), p.to_string_lossy().to_string()));
    relpaths
}

fn is_generated_relpath(path: &Path, generated_relpaths: &[PathBuf]) -> bool {
    generated_relpaths
        .iter()
        .any(|relpath| path == relpath || path.starts_with(relpath))
}

pub fn copy_repo_tree(
    repo_root: &Path,
    destination: &Path,
    generated_paths: &[PathBuf],
) -> Result<()> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let generated_relpaths = generated_relpaths_under_repo(&repo_root, generated_paths);

    fs::create_dir_all(destination)
        .with_context(|| format!("creating destination {:?}", destination))?;

    for entry in WalkDir::new(&repo_root).min_depth(1) {
        let entry = entry?;
        let source = entry.path();
        let relative = source.strip_prefix(&repo_root)?;

        let relative_str = relative.to_string_lossy();
        if is_excluded_relpath(&relative_str) || is_generated_relpath(relative, &generated_relpaths)
        {
            continue;
        }

        let dest = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest).with_context(|| format!("creating directory {:?}", dest))?;
        } else if entry.file_type().is_file() || entry.file_type().is_symlink() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent directory {:?}", parent))?;
            }
            if entry.file_type().is_symlink() {
                let target = fs::read_link(source)?;
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(&target, &dest)
                        .with_context(|| format!("symlinking {:?} -> {:?}", dest, target))?;
                }
                #[cfg(not(unix))]
                {
                    // TODO: Windows symlink support is not implemented.
                    let _ = (target, dest);
                }
            } else {
                fs::copy(source, &dest)
                    .with_context(|| format!("copying {:?} to {:?}", source, dest))?;
            }
        }
    }

    prune_excluded_paths(destination)?;
    Ok(())
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = destination.join(entry.file_name());
        if src.is_dir() {
            copy_dir_contents(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

pub fn export_git_archive(repo_root: &Path, destination: &Path, git_ref: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["archive", "--format=tar", git_ref])
        .output()
        .context("running git archive")?;
    if !output.status.success() {
        bail!(
            "git archive failed for {}: {}",
            git_ref,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::create_dir_all(destination)?;
    let cursor = std::io::Cursor::new(output.stdout);
    let mut archive = tar::Archive::new(cursor);
    archive.unpack(destination)?;
    prune_excluded_paths(destination)?;
    Ok(())
}

pub fn prune_excluded_paths(root: &Path) -> Result<()> {
    let mut paths: Vec<PathBuf> = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok().map(|e| e.path().to_path_buf()))
        .collect();
    paths.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    for path in paths {
        if is_excluded_part(path.file_name().and_then(|n| n.to_str()).unwrap_or("")) {
            if path.is_dir() {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("pruning directory {:?}", path))?;
            } else if path.is_file() || path.is_symlink() {
                fs::remove_file(&path).with_context(|| format!("pruning file {:?}", path))?;
            }
        }
    }
    Ok(())
}

pub fn build_rust_workspace_for_release(
    artifact_root: &Path,
    _target_platform: &str,
) -> Result<()> {
    let rust_dir = artifact_root.join("rust");
    if !rust_dir.join("Cargo.toml").is_file() {
        bail!(
            "Rust workspace missing in release tree: {:?}",
            rust_dir.join("Cargo.toml")
        );
    }

    let manifest_path = rust_dir.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("build")
        .arg("--workspace")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(artifact_root)
        .output()
        .context("running cargo build for workspace")?;
    if !output.status.success() {
        bail!(
            "failed to build Rust workspace for release:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let release_dir = rust_dir.join("target").join("release");
    let mut missing = Vec::new();
    for name in REQUIRED_BINARIES {
        let binary = release_dir.join(name);
        if !binary.is_file() {
            missing.push(name);
        }
    }
    if !missing.is_empty() {
        bail!(
            "Rust build did not produce expected binaries: {:?}",
            missing
        );
    }

    install_rust_binaries_into_release_tree(artifact_root, &release_dir)?;
    prune_rust_target_for_release(&rust_dir)?;
    Ok(())
}

fn install_rust_binaries_into_release_tree(artifact_root: &Path, release_dir: &Path) -> Result<()> {
    let bin_dir = artifact_root.join("bin");
    fs::create_dir_all(&bin_dir).with_context(|| format!("creating bin dir {:?}", bin_dir))?;

    for name in REQUIRED_BINARIES {
        let source = release_dir.join(name);
        let dest = if *name == "ccbr" {
            artifact_root.join(name)
        } else {
            bin_dir.join(name)
        };
        fs::copy(&source, &dest).with_context(|| format!("copying {:?} to {:?}", source, dest))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest, perms)
                .with_context(|| format!("setting permissions on {:?}", dest))?;
        }
    }
    Ok(())
}

fn prune_rust_target_for_release(rust_dir: &Path) -> Result<()> {
    let target_dir = rust_dir.join("target");
    let release_dir = target_dir.join("release");
    if !release_dir.is_dir() {
        return Ok(());
    }

    let keep: std::collections::HashSet<&str> = REQUIRED_BINARIES.iter().copied().collect();
    for entry in fs::read_dir(&release_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if keep.contains(file_name) {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }

    for entry in fs::read_dir(&target_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some("release") {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn build_sidebar_helper_for_release(artifact_root: &Path, target_platform: &str) -> Result<()> {
    let crate_dir = artifact_root.join("tools").join("ccbr-agent-sidebar");
    let output_bin = artifact_root.join("bin").join("ccbr-agent-sidebar");
    if !crate_dir.join("Cargo.toml").is_file() {
        return Ok(());
    }

    if target_platform == "macos" {
        build_macos_universal_sidebar_helper(artifact_root, &crate_dir, &output_bin)?;
    } else {
        build_native_sidebar_helper(artifact_root, &crate_dir, &output_bin)?;
    }

    let sidebar_target = crate_dir.join("target");
    if sidebar_target.exists() {
        fs::remove_dir_all(&sidebar_target)?;
    }
    Ok(())
}

fn build_native_sidebar_helper(
    artifact_root: &Path,
    crate_dir: &Path,
    output_bin: &Path,
) -> Result<()> {
    let source_bin = crate_dir
        .join("target")
        .join("release")
        .join("ccbr-agent-sidebar");
    run_sidebar_cargo_build(artifact_root, crate_dir, None)?;
    if !source_bin.is_file() {
        bail!(
            "sidebar build did not produce expected binary: {:?}",
            source_bin
        );
    }
    fs::create_dir_all(output_bin.parent().unwrap())?;
    fs::copy(&source_bin, output_bin)
        .with_context(|| format!("copying sidebar binary to {:?}", output_bin))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(output_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(output_bin, perms)?;
    }
    Ok(())
}

fn build_macos_universal_sidebar_helper(
    artifact_root: &Path,
    crate_dir: &Path,
    output_bin: &Path,
) -> Result<()> {
    let mut target_bins: Vec<PathBuf> = Vec::new();
    for target in ["x86_64-apple-darwin", "aarch64-apple-darwin"] {
        run_sidebar_cargo_build(artifact_root, crate_dir, Some(target))?;
        let target_bin = crate_dir
            .join("target")
            .join(target)
            .join("release")
            .join("ccbr-agent-sidebar");
        if !target_bin.is_file() {
            bail!(
                "sidebar build did not produce expected {} binary: {:?}",
                target,
                target_bin
            );
        }
        target_bins.push(target_bin);
    }

    fs::create_dir_all(output_bin.parent().unwrap())?;
    let mut cmd = Command::new("lipo");
    cmd.arg("-create").arg("-output").arg(output_bin);
    for bin in &target_bins {
        cmd.arg(bin);
    }
    let output = cmd
        .current_dir(artifact_root)
        .output()
        .context("running lipo")?;
    if !output.status.success() {
        bail!(
            "failed to create macOS universal ccbr-agent-sidebar: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(output_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(output_bin, perms)?;
    }

    verify_macos_universal_sidebar_binary(output_bin)?;
    Ok(())
}

fn run_sidebar_cargo_build(
    artifact_root: &Path,
    crate_dir: &Path,
    target: Option<&str>,
) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(crate_dir.join("Cargo.toml"))
        .current_dir(artifact_root);
    if let Some(target) = target {
        cmd.arg("--target").arg(target);
    }
    let output = cmd.output().context("running sidebar cargo build")?;
    if !output.status.success() {
        bail!(
            "failed to build ccbr-agent-sidebar{} for release:\n{}",
            target
                .map(|t| format!(" for target {}", t))
                .unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn verify_macos_universal_sidebar_binary(output_bin: &Path) -> Result<()> {
    let output = Command::new("file")
        .arg(output_bin)
        .output()
        .context("running file on sidebar binary")?;
    if !output.status.success() {
        bail!(
            "failed to inspect macOS ccbr-agent-sidebar binary: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let description = String::from_utf8_lossy(&output.stdout);
    if !description.contains("universal binary") {
        bail!(
            "macOS ccbr-agent-sidebar is not a universal binary: {}",
            description.trim()
        );
    }
    Ok(())
}

pub fn write_release_metadata(artifact_root: &Path, build_info: &BuildInfo) -> Result<()> {
    let version_text = build_info.version.trim();
    if !version_text.is_empty() {
        fs::write(artifact_root.join("VERSION"), format!("{}\n", version_text))?;
    }
    fs::write(
        artifact_root.join("BUILD_INFO.json"),
        serde_json::to_string_pretty(build_info)? + "\n",
    )?;
    Ok(())
}

pub fn create_tarball(stage_root: &Path, artifact_root: &Path, artifact_path: &Path) -> Result<()> {
    let legacy_alias = stage_root.join(artifact_path.file_name().unwrap());
    if legacy_alias.exists() {
        fs::remove_file(&legacy_alias)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(artifact_root.file_name().unwrap(), &legacy_alias)
            .with_context(|| format!("creating legacy alias symlink {:?}", legacy_alias))?;
    }
    #[cfg(not(unix))]
    {
        // TODO: Windows release path is stubbed.
        bail!("Windows symlink legacy alias not implemented");
    }

    fs::create_dir_all(artifact_path.parent().unwrap())?;
    let file = fs::File::create(artifact_path)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = Builder::new(encoder);
    builder.append_dir_all(artifact_root.file_name().unwrap(), artifact_root)?;
    builder.append_dir_all(legacy_alias.file_name().unwrap(), &legacy_alias)?;
    builder.into_inner()?.finish()?;

    fs::remove_dir_all(stage_root)
        .with_context(|| format!("cleaning up stage root {:?}", stage_root))?;
    Ok(())
}

pub fn write_sha256(artifact_path: &Path, output_path: &Path) -> Result<()> {
    let mut file = fs::File::open(artifact_path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hex::encode(hasher.finalize());
    fs::write(
        output_path,
        format!(
            "{}  {}\n",
            digest,
            artifact_path.file_name().unwrap().to_string_lossy()
        ),
    )?;
    Ok(())
}

pub fn utc_now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn normalize_arch(raw_arch: &str) -> String {
    let text = raw_arch.trim().to_lowercase();
    match text.as_str() {
        "x86_64" | "amd64" => "x86_64".into(),
        "aarch64" | "arm64" => "aarch64".into(),
        _ => text,
    }
}

pub fn normalize_release_platform(raw_system: &str) -> Option<String> {
    match raw_system.trim() {
        "Linux" | "linux" => Some("linux".into()),
        "Darwin" | "macos" => Some("macos".into()),
        _ => None,
    }
}

pub fn release_build_arch(platform_name: &str, machine: &str) -> Option<String> {
    let platform_name = normalize_release_platform(platform_name)?;
    if platform_name == "linux" {
        return Some(normalize_arch(machine));
    }
    if platform_name == "macos" {
        return Some("universal".into());
    }
    None
}

pub fn release_artifact_basename(platform_name: &str, machine: &str) -> Option<String> {
    let platform_name = normalize_release_platform(platform_name)?;
    if platform_name == "linux" {
        let arch = normalize_arch(machine);
        Some(format!("ccbr-linux-{}", arch))
    } else if platform_name == "macos" {
        Some("ccbr-macos-universal".into())
    } else {
        None
    }
}

pub fn release_artifact_name(platform_name: &str, machine: &str) -> Option<String> {
    release_artifact_basename(platform_name, machine).map(|b| format!("{}.tar.gz", b))
}

fn is_excluded_relpath(value: &str) -> bool {
    Path::new(value.trim())
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(is_excluded_part)
}
