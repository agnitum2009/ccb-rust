use camino::{Utf8Path, Utf8PathBuf};
use serde_json::Map;

use crate::Result;
use ccb_storage::atomic::atomic_write_json;

const CCB_FINISH_HOOK_NAME: &str = "ccb-provider-finish-hook";
const CCB_ACTIVITY_HOOK_NAME: &str = "ccb-provider-activity-hook";

const CLAUDE_ACTIVITY_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "Notification",
    "PostToolUse",
    "Stop",
];

pub fn build_hook_command(
    provider: &str,
    script_path: impl AsRef<Utf8Path>,
    python_executable: &str,
    completion_dir: impl AsRef<Utf8Path>,
    agent_name: &str,
    workspace_path: impl AsRef<Utf8Path>,
) -> String {
    let script = expand_user_path(script_path.as_ref());
    let completion = expand_user_path(completion_dir.as_ref());
    let workspace = expand_user_path(workspace_path.as_ref());
    let mut parts: Vec<&str> = Vec::new();
    // When `python_executable` is empty the hook is a native binary invoked
    // directly (no interpreter prefix); otherwise prefix it as before.
    if !python_executable.is_empty() {
        parts.push(python_executable);
    }
    parts.extend_from_slice(&[
        script.as_str(),
        "--provider",
        provider,
        "--completion-dir",
        completion.as_str(),
        "--agent-name",
        agent_name,
        "--workspace",
        workspace.as_str(),
    ]);
    shell_quote_all(&parts)
}

pub fn build_activity_hook_command(
    provider: &str,
    script_path: impl AsRef<Utf8Path>,
    python_executable: &str,
    project_id: &str,
    agent_name: &str,
    runtime_dir: impl AsRef<Utf8Path>,
    workspace_path: impl AsRef<Utf8Path>,
) -> String {
    let script = expand_user_path(script_path.as_ref());
    let runtime = expand_user_path(runtime_dir.as_ref());
    let workspace = expand_user_path(workspace_path.as_ref());
    let mut parts: Vec<&str> = Vec::new();
    if !python_executable.is_empty() {
        parts.push(python_executable);
    }
    parts.extend_from_slice(&[
        script.as_str(),
        "--provider",
        provider,
        "--project-id",
        project_id,
        "--agent-name",
        agent_name,
        "--runtime-dir",
        runtime.as_str(),
        "--workspace",
        workspace.as_str(),
    ]);
    shell_quote_all(&parts)
}

pub fn install_workspace_completion_hooks(
    provider: &str,
    workspace_path: impl AsRef<Utf8Path>,
    home_root: Option<&Utf8Path>,
    command: &str,
) -> Option<Utf8PathBuf> {
    install_workspace_completion_hooks_with_profile(
        provider,
        workspace_path,
        home_root,
        command,
        None,
    )
}

/// Same as [`install_workspace_completion_hooks`] but accepts an optional
/// `resolved_profile` argument for Python signature parity. The profile is
/// intentionally ignored, matching the Python implementation.
pub fn install_workspace_completion_hooks_with_profile(
    provider: &str,
    workspace_path: impl AsRef<Utf8Path>,
    home_root: Option<&Utf8Path>,
    command: &str,
    _resolved_profile: Option<&serde_json::Value>,
) -> Option<Utf8PathBuf> {
    let normalized = provider.trim().to_lowercase();
    let home_root = home_root?;
    match normalized.as_str() {
        "claude" => {
            let settings_path = install_claude_hooks(home_root, command);
            trust_claude_workspace(home_root, workspace_path.as_ref());
            Some(settings_path)
        }
        "gemini" => {
            let settings_path = install_gemini_hooks(home_root, command);
            trust_gemini_workspace(home_root, workspace_path.as_ref());
            Some(settings_path)
        }
        _ => None,
    }
}

pub fn install_workspace_activity_hooks(
    provider: &str,
    workspace_path: impl AsRef<Utf8Path>,
    home_root: Option<&Utf8Path>,
    command: &str,
) -> Option<Utf8PathBuf> {
    let normalized = provider.trim().to_lowercase();
    let home_root = home_root?;
    match normalized.as_str() {
        "claude" => {
            let settings_path = install_claude_activity_hooks(home_root, command);
            trust_claude_workspace(home_root, workspace_path.as_ref());
            Some(settings_path)
        }
        _ => None,
    }
}

pub fn install_claude_hooks(home_root: &Utf8Path, command: &str) -> Utf8PathBuf {
    let settings_path = claude_settings_path(home_root);
    let mut data = load_json(&settings_path).unwrap_or_default();
    let hooks = hooks_payload(&mut data);
    let event_name = "Stop";
    let groups = event_groups(hooks, event_name);
    let groups = prune_ccb_managed_hook_groups(groups, command, CCB_FINISH_HOOK_NAME);
    let groups = if !claude_event_has_command(&groups, command) {
        let mut groups = groups;
        groups.push(command_hook_group(command));
        groups
    } else {
        groups
    };
    hooks.insert(event_name.into(), serde_json::Value::Array(groups));
    atomic_write_json(&settings_path, &data).ok();
    settings_path
}

pub fn install_claude_activity_hooks(home_root: &Utf8Path, command: &str) -> Utf8PathBuf {
    let settings_path = claude_settings_path(home_root);
    let mut data = load_json(&settings_path).unwrap_or_default();
    let hooks = hooks_payload(&mut data);
    for &event_name in CLAUDE_ACTIVITY_EVENTS {
        let groups = event_groups(hooks, event_name);
        let groups = prune_ccb_managed_hook_groups(groups, command, CCB_ACTIVITY_HOOK_NAME);
        let groups = if !claude_event_has_command(&groups, command) {
            let mut groups = groups;
            groups.push(command_hook_group(command));
            groups
        } else {
            groups
        };
        hooks.insert(event_name.into(), serde_json::Value::Array(groups));
    }
    atomic_write_json(&settings_path, &data).ok();
    settings_path
}

pub fn trust_claude_workspace(home_root: &Utf8Path, workspace_path: &Utf8Path) -> Utf8PathBuf {
    let trust_path = claude_trust_path(home_root);
    let mut data = load_json(&trust_path).unwrap_or_default();
    let key = workspace_key(workspace_path);
    let record = data
        .entry(key.clone())
        .or_insert_with(|| serde_json::Value::Object(Map::new()));
    if let Some(obj) = record.as_object_mut() {
        obj.insert(
            "hasTrustDialogAccepted".into(),
            serde_json::Value::Bool(true),
        );
    }
    let projects = data
        .entry("projects")
        .or_insert_with(|| serde_json::Value::Object(Map::new()));
    if let Some(projects_obj) = projects.as_object_mut() {
        let project_record = projects_obj
            .entry(key.clone())
            .or_insert_with(|| serde_json::Value::Object(Map::new()));
        if let Some(obj) = project_record.as_object_mut() {
            obj.insert(
                "hasTrustDialogAccepted".into(),
                serde_json::Value::Bool(true),
            );
        }
    }
    atomic_write_json(&trust_path, &data).ok();
    trust_path
}

pub fn claude_event_has_command(groups: &[serde_json::Value], command: &str) -> bool {
    for group in groups {
        let Some(obj) = group.as_object() else {
            continue;
        };
        let Some(hooks) = obj.get("hooks").and_then(|v| v.as_array()) else {
            continue;
        };
        for hook in hooks {
            let Some(hook_obj) = hook.as_object() else {
                continue;
            };
            let hook_type = hook_obj
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_lowercase());
            if hook_type != Some("command".to_string()) {
                continue;
            }
            if hook_obj
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                == Some(command)
            {
                return true;
            }
        }
    }
    false
}

fn prune_ccb_managed_hook_groups(
    groups: Vec<serde_json::Value>,
    current_command: &str,
    managed_hook_name: &str,
) -> Vec<serde_json::Value> {
    let mut pruned = Vec::new();
    for group in groups {
        let Some(obj) = group.as_object() else {
            pruned.push(group);
            continue;
        };
        let Some(hooks) = obj.get("hooks").and_then(|v| v.as_array()) else {
            pruned.push(group);
            continue;
        };
        let kept_hooks: Vec<serde_json::Value> = hooks
            .iter()
            .filter(|hook| !is_stale_ccb_managed_hook(hook, current_command, managed_hook_name))
            .cloned()
            .collect();
        if !kept_hooks.is_empty() {
            let mut next_group = obj.clone();
            next_group.insert("hooks".into(), serde_json::Value::Array(kept_hooks));
            pruned.push(serde_json::Value::Object(next_group));
        }
    }
    pruned
}

fn is_stale_ccb_managed_hook(
    hook: &serde_json::Value,
    current_command: &str,
    managed_hook_name: &str,
) -> bool {
    let Some(obj) = hook.as_object() else {
        return false;
    };
    let hook_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase());
    if hook_type != Some("command".to_string()) {
        return false;
    }
    let command = obj
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    command.contains(managed_hook_name) && command != current_command
}

fn command_hook_group(command: &str) -> serde_json::Value {
    serde_json::json!({
        "hooks": [
            {
                "type": "command",
                "command": command,
            }
        ]
    })
}

pub fn install_gemini_hooks(home_root: &Utf8Path, command: &str) -> Utf8PathBuf {
    let settings_path = gemini_settings_path(home_root);
    let mut data = load_json(&settings_path).unwrap_or_default();
    let hooks = hooks_payload(&mut data);
    let after_agent = hooks
        .entry("AfterAgent")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    let after_agent_array = after_agent.as_array_mut().unwrap();
    if !gemini_event_has_command(after_agent_array, command) {
        after_agent_array.push(serde_json::json!({
            "matcher": "*",
            "hooks": [
                {
                    "type": "command",
                    "command": command,
                }
            ]
        }));
    }
    atomic_write_json(&settings_path, &data).ok();
    settings_path
}

pub fn trust_gemini_workspace(home_root: &Utf8Path, workspace_path: &Utf8Path) -> Utf8PathBuf {
    let trust_path = gemini_trust_path(home_root);
    let mut data = load_json(&trust_path).unwrap_or_default();
    data.insert(
        workspace_key(workspace_path),
        serde_json::Value::String("TRUST_FOLDER".into()),
    );
    atomic_write_json(&trust_path, &data).ok();
    trust_path
}

pub fn gemini_event_has_command(groups: &[serde_json::Value], command: &str) -> bool {
    for group in groups {
        let Some(obj) = group.as_object() else {
            continue;
        };
        let Some(hooks) = obj.get("hooks").and_then(|v| v.as_array()) else {
            continue;
        };
        for hook in hooks {
            let Some(hook_obj) = hook.as_object() else {
                continue;
            };
            let hook_type = hook_obj
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_lowercase());
            if hook_type != Some("command".to_string()) {
                continue;
            }
            if hook_obj
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                == Some(command)
            {
                return true;
            }
        }
    }
    false
}

fn claude_settings_path(home_root: &Utf8Path) -> Utf8PathBuf {
    expand_user_path(home_root)
        .join(".claude")
        .join("settings.json")
}

fn claude_trust_path(home_root: &Utf8Path) -> Utf8PathBuf {
    expand_user_path(home_root).join(".claude.json")
}

fn gemini_settings_path(home_root: &Utf8Path) -> Utf8PathBuf {
    expand_user_path(home_root)
        .join(".gemini")
        .join("settings.json")
}

fn gemini_trust_path(home_root: &Utf8Path) -> Utf8PathBuf {
    expand_user_path(home_root)
        .join(".gemini")
        .join("trustedFolders.json")
}

pub fn load_json(path: &Utf8Path) -> Result<Map<String, serde_json::Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(Map::new()),
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Ok(Map::new()),
    };
    Ok(value.as_object().cloned().unwrap_or_default())
}

pub fn save_json(path: &Utf8Path, data: &Map<String, serde_json::Value>) -> Result<Utf8PathBuf> {
    atomic_write_json(path, data)?;
    Ok(path.to_path_buf())
}

/// Return the expanded layout root for Claude hooks.
///
/// Mirrors Python `settings_runtime.claude.claude_hook_home_layout`. The Rust
/// version returns the expanded home root path (the Python version returns a
/// `ClaudeHomeLayout` dataclass).
pub fn claude_hook_home_layout(home_root: &Utf8Path) -> Utf8PathBuf {
    expand_user_path(home_root)
}

fn hooks_payload(data: &mut Map<String, serde_json::Value>) -> &mut Map<String, serde_json::Value> {
    if !data.contains_key("hooks") || !data["hooks"].is_object() {
        data.insert("hooks".into(), serde_json::Value::Object(Map::new()));
    }
    data.get_mut("hooks").unwrap().as_object_mut().unwrap()
}

fn event_groups(
    hooks: &Map<String, serde_json::Value>,
    event_name: &str,
) -> Vec<serde_json::Value> {
    hooks
        .get(event_name)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

pub fn workspace_key(workspace_path: &Utf8Path) -> String {
    let expanded = expand_user_path(workspace_path);
    expanded
        .canonicalize()
        .ok()
        .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
        .unwrap_or(expanded)
        .to_string()
}

fn expand_user_path(path: &Utf8Path) -> Utf8PathBuf {
    if let Some(rest) = path.as_str().strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return Utf8PathBuf::from(format!("{home}{rest}"));
        }
    }
    path.to_path_buf()
}

fn shell_quote_all(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|p| shell_quote(p))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-:=@+%".contains(c))
    {
        return s.into();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_build_hook_command() {
        let command = build_hook_command(
            "claude",
            Utf8Path::new("/tmp/bin/ccb-provider-finish-hook"),
            "/usr/bin/python3",
            Utf8Path::new("/tmp/completion"),
            "agent1",
            Utf8Path::new("/tmp/workspace"),
        );
        assert!(command.contains("--provider claude"));
        assert!(command.contains("--agent-name agent1"));
        assert!(command.contains("--completion-dir"));
        assert!(command.contains("--workspace"));
    }

    #[test]
    fn test_build_activity_hook_command() {
        let command = build_activity_hook_command(
            "claude",
            Utf8Path::new("/tmp/bin/ccb-provider-activity-hook"),
            "/usr/bin/python3",
            "project-1",
            "agent1",
            Utf8Path::new("/tmp/runtime"),
            Utf8Path::new("/tmp/workspace"),
        );
        assert!(command.contains("--provider claude"));
        assert!(command.contains("--project-id project-1"));
        assert!(command.contains("--agent-name agent1"));
        assert!(command.contains("--runtime-dir"));
        assert!(command.contains("--workspace"));
    }

    #[test]
    fn test_install_claude_hooks_writes_stop_hook() {
        let dir = TempDir::new().unwrap();
        let home_root = Utf8Path::from_path(dir.path()).unwrap().join("claude-home");
        let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
        let command = "/usr/bin/python3 /tmp/ccb-provider-finish-hook --provider claude";

        let settings_path =
            install_workspace_completion_hooks("claude", &workspace, Some(&home_root), command)
                .unwrap();

        assert_eq!(
            settings_path,
            home_root.join(".claude").join("settings.json")
        );
        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(data["hooks"]["Stop"][0]["hooks"][0]["command"], command);
    }

    #[test]
    fn test_install_claude_hooks_prunes_stale_hooks() {
        let dir = TempDir::new().unwrap();
        let home_root = Utf8Path::from_path(dir.path()).unwrap().join("claude-home");
        let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
        std::fs::create_dir_all(&home_root).unwrap();
        let settings_path = home_root.join(".claude").join("settings.json");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let command = "/usr/bin/python3 /current/bin/ccb-provider-finish-hook --provider claude";
        let stale_command = "/usr/bin/python3 /old/bin/ccb-provider-finish-hook --provider claude";
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&json!({
                "hooks": {
                    "Stop": [
                        {"hooks": [{"type": "command", "command": "echo existing"}]},
                        {"hooks": [{"type": "command", "command": stale_command}]},
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        install_workspace_completion_hooks("claude", &workspace, Some(&home_root), command);

        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let commands: Vec<String> = data["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["hooks"].as_array().unwrap().iter())
            .filter_map(|hook| hook["command"].as_str().map(|s| s.to_string()))
            .collect();
        assert_eq!(commands, vec!["echo existing", command]);
    }

    #[test]
    fn test_install_gemini_hooks_writes_after_agent_hook() {
        let dir = TempDir::new().unwrap();
        let home_root = Utf8Path::from_path(dir.path()).unwrap().join("gemini-home");
        let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
        let command = "/usr/bin/python3 /tmp/ccb-provider-finish-hook --provider gemini";

        let settings_path =
            install_workspace_completion_hooks("gemini", &workspace, Some(&home_root), command)
                .unwrap();

        assert_eq!(
            settings_path,
            home_root.join(".gemini").join("settings.json")
        );
        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(data["hooks"]["AfterAgent"][0]["matcher"], "*");
        assert_eq!(
            data["hooks"]["AfterAgent"][0]["hooks"][0]["command"],
            command
        );
    }
}
