//! Mirrors Python `lib/provider_backends/gemini/launcher_runtime/service.py`.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_provider_core::caller_env::{caller_context_env, export_env_clause, join_env_prefix};
use ccbr_provider_core::runtime_shared::{apply_provider_command_template, provider_start_parts};
use sha2::Digest;

use super::{gemini_layout_for_home, GeminiHomeLayout};

/// Start-command options specific to Gemini.
#[derive(Debug, Clone, Default)]
pub struct GeminiStartCommand {
    pub restore: bool,
    pub auto_permission: bool,
    pub provider_command_template: Option<String>,
}

/// Restore target for a Gemini launch.
#[derive(Debug, Clone)]
pub struct GeminiRestoreTarget {
    pub run_cwd: Utf8PathBuf,
    pub has_history: bool,
}

/// Resolve whether Gemini should resume an existing session.
pub fn resolve_gemini_restore_target(
    _spec: &ccbr_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    restore: bool,
    workspace_path: Option<&Utf8Path>,
) -> GeminiRestoreTarget {
    let (session_root, session_work_dir) = managed_session_root_and_work_dir(runtime_dir);
    let workspace_path = session_work_dir
        .or_else(|| workspace_path.map(|p| p.to_path_buf()))
        .unwrap_or_else(|| Utf8PathBuf::from("."));

    let default = GeminiRestoreTarget {
        run_cwd: workspace_path.clone(),
        has_history: false,
    };

    if !restore {
        return default;
    }

    let tmp_root = session_root.unwrap_or_else(|| resolve_gemini_home_layout(runtime_dir).tmp_root);
    let chats_dir = tmp_root.join(project_hash(&workspace_path)).join("chats");

    let has_history = chats_dir
        .read_dir()
        .ok()
        .map(|mut entries| {
            entries.any(|e| {
                e.ok().map(|entry| {
                    let path = entry.path();
                    path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json")
                }) == Some(true)
            })
        })
        .unwrap_or(false);

    GeminiRestoreTarget {
        run_cwd: workspace_path,
        has_history,
    }
}

fn managed_session_root_and_work_dir(
    runtime_dir: &Utf8Path,
) -> (Option<std::path::PathBuf>, Option<Utf8PathBuf>) {
    let session_path =
        match crate::session_paths::session_file_for_runtime_dir("gemini", runtime_dir) {
            Some(p) => p,
            None => return (None, None),
        };
    let payload = match crate::session_paths::read_session_payload(&session_path) {
        Some(p) => p,
        None => return (None, None),
    };
    let work_dir = payload
        .get("work_dir")
        .and_then(|v| v.as_str())
        .map(Utf8PathBuf::from)
        .filter(|p| !p.as_str().is_empty());
    let tmp_root = payload
        .get("gemini_root")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());

    let managed_home = resolve_gemini_home_layout(runtime_dir).home_root;
    let managed_tmp = managed_home.join(".gemini").join("tmp");
    let tmp_root =
        tmp_root.filter(|root| root.starts_with(&managed_tmp) || root.starts_with(&managed_home));
    (tmp_root, work_dir)
}

/// Build the shell command that launches a Gemini runtime pane.
pub fn build_start_cmd(
    command: &GeminiStartCommand,
    spec: &ccbr_agents::models::AgentSpec,
    runtime_dir: &Utf8Path,
    launch_session_id: &str,
    prepared_state: Option<&HashMap<String, String>>,
) -> anyhow::Result<String> {
    let prepared_state = prepared_state.cloned().unwrap_or_default();
    let project_root = path_or_none(prepared_state.get("project_root")).ok_or_else(|| {
        anyhow::anyhow!("Gemini launch requires prepare_launch_context before build_start_cmd")
    })?;
    let workspace_path = path_or_none(prepared_state.get("workspace_path"));

    let layout = resolve_gemini_home_layout(runtime_dir);
    let home_overrides = gemini_home_overrides(&layout, runtime_dir, Some(&project_root));

    let restore_target = resolve_gemini_restore_target(
        spec,
        runtime_dir,
        command.restore,
        workspace_path.as_deref(),
    );

    let mut cmd_parts = provider_start_parts("gemini");
    if command.auto_permission {
        cmd_parts.push("--yolo".to_string());
    }
    if restore_target.has_history {
        cmd_parts.extend(["--resume".to_string(), "latest".to_string()]);
    }
    cmd_parts.extend(spec.startup_args.iter().cloned());

    let cmd = cmd_parts
        .iter()
        .map(|p| shell_quote(p))
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = apply_provider_command_template(&cmd, command.provider_command_template.as_deref())?;

    let env_prefix = join_env_prefix(&[
        &build_gemini_env_prefix(
            load_resolved_provider_profile(runtime_dir).as_ref(),
            Some(&spec.env),
        ),
        &export_env_clause(&ccbr_provider_core::caller_env::provider_user_session_env()),
        &export_env_clause(&home_overrides),
        &export_env_clause(&caller_context_env(
            &spec.name,
            runtime_dir.as_std_path(),
            launch_session_id,
        )),
    ]);

    if env_prefix.is_empty() {
        Ok(cmd)
    } else {
        Ok(format!("{}; {}", env_prefix, cmd))
    }
}

fn resolve_gemini_home_layout(runtime_dir: &Utf8Path) -> GeminiHomeLayout {
    let home = crate::session_paths::state_dir_for_runtime_dir(runtime_dir)
        .map(|p| p.join("home"))
        .unwrap_or_else(|| runtime_dir.as_std_path().join("gemini-home"));
    gemini_layout_for_home(&home)
}

fn gemini_home_overrides(
    layout: &GeminiHomeLayout,
    runtime_dir: &Utf8Path,
    project_root: Option<&Utf8Path>,
) -> HashMap<String, String> {
    let cache_root = gemini_cache_root(project_root, runtime_dir);
    let mut overrides = HashMap::new();
    overrides.insert(
        "HOME".to_string(),
        layout.home_root.to_string_lossy().to_string(),
    );
    overrides.insert(
        "GEMINI_CLI_HOME".to_string(),
        layout.home_root.to_string_lossy().to_string(),
    );
    overrides.insert(
        "GEMINI_ROOT".to_string(),
        layout.tmp_root.to_string_lossy().to_string(),
    );
    overrides.insert(
        "NPM_CONFIG_CACHE".to_string(),
        cache_root.join("npm").to_string(),
    );
    overrides.insert(
        "npm_config_cache".to_string(),
        cache_root.join("npm").to_string(),
    );
    overrides.insert(
        "XDG_CACHE_HOME".to_string(),
        cache_root.join("xdg").to_string(),
    );
    overrides
}

fn gemini_cache_root(project_root: Option<&Utf8Path>, runtime_dir: &Utf8Path) -> Utf8PathBuf {
    project_root
        .map(|p| {
            let id = project_hash(p);
            cache_home()
                .join("ccb")
                .join("projects")
                .join(&id[..id.len().min(16)])
                .join("provider-cache")
                .join("gemini")
        })
        .unwrap_or_else(|| runtime_dir.join("rebuildable-cache").join("gemini"))
}

fn cache_home() -> Utf8PathBuf {
    std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| {
            Utf8PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()))
                .join(".cache")
        })
}

fn project_hash(project_root: &Utf8Path) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(project_root.as_str().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn build_gemini_env_prefix(
    profile: Option<&ccbr_provider_profiles::models::ResolvedProviderProfile>,
    extra_env: Option<&HashMap<String, String>>,
) -> String {
    let api_keys = ccbr_provider_profiles::provider_api_env_keys("gemini");
    let mut explicit = HashMap::new();
    if let Some(profile) = profile {
        explicit.extend(
            profile
                .env
                .iter()
                .filter(|(k, _)| api_keys.contains(k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
    if let Some(extra) = extra_env {
        explicit.extend(
            extra
                .iter()
                .filter(|(k, _)| api_keys.contains(k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }

    let mut parts = Vec::new();
    if profile.map(|p| !p.inherit_api).unwrap_or(false) {
        let mut keys: Vec<_> = api_keys.iter().cloned().collect();
        keys.sort();
        for key in keys {
            parts.push(format!("unset {}", key));
        }
    }
    let export = export_env_clause(&explicit);
    if !export.is_empty() {
        parts.push(export);
    }
    parts.join("; ")
}

fn load_resolved_provider_profile(
    runtime_dir: &Utf8Path,
) -> Option<ccbr_provider_profiles::models::ResolvedProviderProfile> {
    let path = runtime_dir.join("provider-profile.json");
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let record = value.as_object().cloned()?;
    ccbr_provider_profiles::models::ResolvedProviderProfile::from_record(&record).ok()
}

fn path_or_none(value: Option<&String>) -> Option<Utf8PathBuf> {
    let raw = value.map(|s| s.trim()).filter(|s| !s.is_empty())?;
    Some(Utf8PathBuf::from(raw))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || "_-.,/:~=@%".contains(c))
    {
        return value.to_string();
    }
    let mut out = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
