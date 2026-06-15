//! Mirrors Python `lib/cli/management_runtime/commands_runtime/version.py`.

use serde_json::{Map, Value};
use std::path::Path;

use super::matching::{is_newer_version, latest_version};
use crate::management_runtime::install::find_install_dir;
use crate::management_runtime::versioning::{
    format_version_info, get_available_versions, get_remote_version_info, get_version_info,
};

/// Print local version info and a remote update check.
///
/// Mirrors Python `cmd_version(args, script_root)`.
pub fn cmd_version(_args: &Value, script_root: &Path) -> i32 {
    let install_dir = find_install_dir(script_root);
    let local_info = get_version_info(&install_dir);
    let local_str = format_version_info(&local_info);
    let install_mode =
        str_field(&local_info, "install_mode").unwrap_or_else(|| "unknown".to_string());
    let source_kind =
        str_field(&local_info, "source_kind").unwrap_or_else(|| "unknown".to_string());
    let channel = str_field(&local_info, "channel").unwrap_or_else(|| "unknown".to_string());

    println!("ccb (Claude Code Bridge) {}", local_str);
    println!("Install path: {}", install_dir.display());
    println!("Install mode: {}", install_mode);
    println!("Install source: {}", source_kind);
    println!("Channel: {}", channel);
    if str_field(&local_info, "platform").is_some()
        || str_field(&local_info, "arch").is_some()
        || str_field(&local_info, "build_time").is_some()
    {
        println!(
            "Build: {} {} {}",
            str_field(&local_info, "platform")
                .as_deref()
                .unwrap_or("unknown"),
            str_field(&local_info, "arch")
                .as_deref()
                .unwrap_or("unknown"),
            str_field(&local_info, "build_time")
                .as_deref()
                .unwrap_or("unknown"),
        );
    }

    println!("\nChecking for updates...");
    if is_source_install(&local_info, &install_dir) {
        print_source_update_status(&local_info);
    } else if install_dir.join(".git").exists() {
        print_git_update_status(&local_info);
    } else {
        print_release_update_status(&local_info);
    }
    0
}

fn is_source_install(local_info: &Map<String, Value>, install_dir: &Path) -> bool {
    if str_field(local_info, "install_mode").as_deref() == Some("source") {
        return true;
    }
    if str_field(local_info, "source_kind").as_deref() == Some("source") {
        return true;
    }
    install_dir.join(".git").exists()
}

fn print_source_update_status(local_info: &Map<String, Value>) {
    let remote_info = match get_remote_version_info() {
        Some(info) => info,
        None => {
            println!("⚠️  Unable to check source updates (network error)");
            println!("   Run: ccb update  to install the latest stable release");
            return;
        }
    };
    let local_commit = str_field(local_info, "commit");
    let remote_sha = remote_info.get("commit").and_then(|v| v.as_str());
    if let (Some(local), Some(remote)) = (local_commit.as_deref(), remote_sha) {
        if local == remote {
            println!("✅ Up to date");
            println!("   Run: ccb update  to switch this install to the latest stable release");
            return;
        }
        let date = remote_info
            .get("date")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let remote_str = format!("{} {}", remote, date).trim().to_string();
        println!("📦 Source update available: {}", remote_str);
        println!("   Use: git pull  (or switch commits in your checkout)");
        println!(
            "   Rerun: ./install.sh install  if you want the global install to stay in source/dev mode"
        );
        println!("   Run: ccb update  to switch the global install to the latest stable release");
        return;
    }
    println!("⚠️  Unable to compare source revisions");
    println!("   Run: ccb update  to install the latest stable release");
}

fn print_git_update_status(local_info: &Map<String, Value>) {
    let remote_info = match get_remote_version_info() {
        Some(info) => info,
        None => {
            println!("⚠️  Unable to check for updates (network error)");
            return;
        }
    };
    let local_commit = str_field(local_info, "commit");
    let remote_sha = remote_info.get("commit").and_then(|v| v.as_str());
    if let (Some(local), Some(remote)) = (local_commit.as_deref(), remote_sha) {
        if local == remote {
            println!("✅ Up to date");
            return;
        }
        let date = remote_info
            .get("date")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let remote_str = format!("{} {}", remote, date).trim().to_string();
        println!("📦 Update available: {}", remote_str);
        println!("   Run: ccb update");
        return;
    }
    println!("⚠️  Unable to compare versions");
}

fn print_release_update_status(local_info: &Map<String, Value>) {
    let versions = get_available_versions(15.0, 30.0);
    let latest = match latest_version(&versions) {
        Some(v) => v,
        None => {
            println!("⚠️  Unable to check release updates");
            return;
        }
    };
    let current = str_field(local_info, "version").unwrap_or_default();
    if !current.is_empty() && !is_newer_version(&latest, &current) {
        println!("✅ Up to date (latest release: v{})", latest);
        return;
    }
    if !current.is_empty() {
        println!("📦 Release update available: v{}", latest);
        println!("   Run: ccb update");
        return;
    }
    println!("📦 Latest release: v{}", latest);
    println!("   Run: ccb update");
}

fn str_field(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
