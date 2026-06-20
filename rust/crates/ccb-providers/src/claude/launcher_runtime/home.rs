//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/home.py`.
//!
//! Home-layout resolution for an isolated Claude runtime.

use camino::{Utf8Path, Utf8PathBuf};
use ccb_provider_profiles::models::ResolvedProviderProfile;

use crate::claude::home_layout::{claude_layout_for_home, ClaudeHomeLayout};

/// Resolve the isolated Claude home layout for a runtime directory.
///
/// Mirrors Python `resolve_claude_home_layout`.
pub fn resolve_claude_home_layout(
    runtime_dir: &Utf8Path,
    profile: Option<&ResolvedProviderProfile>,
) -> ClaudeHomeLayout {
    if let Some(home) = profile_runtime_home(profile) {
        return claude_layout_for_home(home);
    }

    let managed_home = managed_isolated_home(runtime_dir);
    if let Some(existing) = existing_layout(runtime_dir, &managed_home) {
        return existing;
    }

    claude_layout_for_home(managed_home)
}

fn profile_runtime_home(profile: Option<&ResolvedProviderProfile>) -> Option<Utf8PathBuf> {
    let home = profile?.runtime_home.as_deref()?;
    let home = home.trim();
    if home.is_empty() {
        return None;
    }
    Some(Utf8PathBuf::from(home))
}

fn managed_isolated_home(runtime_dir: &Utf8Path) -> Utf8PathBuf {
    runtime_dir.join("home")
}

fn existing_layout(runtime_dir: &Utf8Path, managed_home: &Utf8Path) -> Option<ClaudeHomeLayout> {
    // If the managed home already has a settings file, use it.
    let managed_settings = managed_home.join(".claude").join("settings.json");
    if managed_settings.exists() {
        return Some(claude_layout_for_home(managed_home));
    }

    // Otherwise, look for a pre-existing home directory inside the runtime.
    let candidates: Vec<Utf8PathBuf> = vec![
        runtime_dir.join("home"),
        runtime_dir.join(".claude").join("home"),
    ];
    for candidate in candidates {
        if candidate.join(".claude").join("settings.json").exists() {
            return Some(claude_layout_for_home(&candidate));
        }
    }
    None
}
