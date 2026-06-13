use std::collections::{HashMap, HashSet};

use crate::user_session::USER_SESSION_TRANSPORT_ENV_KEYS;

pub const CONTROL_PLANE_ALLOWLIST: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CCB_BACKEND_ENV",
    "CCB_CCBD_FAULTHANDLER",
    "CCB_CCBD_MIN_POLL_INTERVAL_S",
    "CCB_DEBUG",
    "CCB_KEEPER_PID",
    "CCB_KEYCHAIN_SERVICE_OVERRIDE",
    "CCB_LANG",
    "CCB_NO_ATTACH",
    "CCB_REPLY_LANG",
    "CCB_STDIN_ENCODING",
    "CCB_VERSION",
    "DBUS_SESSION_BUS_ADDRESS",
    "DESKTOP_SESSION",
    "DISPLAY",
    "GEMINI_API_KEY",
    "GEMINI_MODEL",
    "GOOGLE_API_BASE",
    "GOOGLE_API_KEY",
    "GOOGLE_GEMINI_BASE_URL",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "HOME",
    "LANG",
    "LC_ALL",
    "LC_MESSAGES",
    "LOCALAPPDATA",
    "OPENAI_API_BASE",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_ORG_ID",
    "OPENAI_ORGANIZATION",
    "PATH",
    "PYTHONUNBUFFERED",
    "SHELL",
    "SSH_AUTH_SOCK",
    "SYSTEMROOT",
    "TERM",
    "TMP",
    "TEMP",
    "TMPDIR",
    "USER",
    "USERPROFILE",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_CURRENT_DESKTOP",
    "XDG_DATA_HOME",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_TYPE",
    "XAUTHORITY",
    "WAYLAND_DISPLAY",
];

pub const CONTROL_PLANE_BLOCKED_PREFIXES: &[&str] = &[
    "CODEX_",
    "CLAUDE_",
    "GEMINI_",
    "OPENCODE_",
    "DROID_",
    "CCB_CALLER_",
];

pub const CONTROL_PLANE_BLOCKED_EXACT: &[&str] = &[
    "CCB_SESSION_FILE",
    "CCB_SESSION_ID",
    "CCB_TMUX_SOCKET",
    "CCB_TMUX_SOCKET_PATH",
    "PYTHONPATH",
    "TMUX",
    "TMUX_PANE",
];

pub fn control_plane_env(
    extra: Option<&HashMap<String, Option<String>>>,
) -> HashMap<String, String> {
    let allowlist: HashSet<&str> = CONTROL_PLANE_ALLOWLIST.iter().copied().collect();
    let blocked_exact: HashSet<&str> = CONTROL_PLANE_BLOCKED_EXACT.iter().copied().collect();
    let transport_keys: HashSet<&str> = USER_SESSION_TRANSPORT_ENV_KEYS.iter().copied().collect();

    let mut env = HashMap::new();
    for (key, value) in std::env::vars() {
        if blocked_exact.contains(key.as_str()) {
            continue;
        }
        if allowlist.contains(key.as_str()) || transport_keys.contains(key.as_str()) {
            env.insert(key, value);
            continue;
        }
        if CONTROL_PLANE_BLOCKED_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            continue;
        }
        if key == "PYTHONPATH" {
            continue;
        }
        if key.starts_with("PYTHON") || key.starts_with("VIRTUAL_ENV") || key.starts_with("CONDA") {
            env.insert(key, value);
        }
    }

    if let Some(extra) = extra {
        for (key, value) in extra {
            match value {
                Some(v) => {
                    env.insert(key.clone(), v.clone());
                }
                None => {
                    env.remove(key);
                }
            }
        }
    }

    env
}
