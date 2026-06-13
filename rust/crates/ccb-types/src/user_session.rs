use std::collections::{HashMap, HashSet};

pub const NETWORK_PROXY_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "WS_PROXY",
    "WSS_PROXY",
    "ws_proxy",
    "wss_proxy",
    "NPM_CONFIG_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_NO_PROXY",
    "npm_config_proxy",
    "npm_config_https_proxy",
    "npm_config_no_proxy",
    "YARN_PROXY",
    "YARN_HTTPS_PROXY",
    "YARN_NO_PROXY",
    "yarn_proxy",
    "yarn_https_proxy",
    "yarn_no_proxy",
    "BUNDLE_HTTPS_PROXY",
    "BUNDLE_NO_PROXY",
    "bundle_https_proxy",
    "bundle_no_proxy",
];

pub const TRUST_STORE_ENV_KEYS: &[&str] = &[
    "CODEX_CA_CERTIFICATE",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "NPM_CONFIG_CAFILE",
    "npm_config_cafile",
];

pub const DESKTOP_SESSION_ENV_KEYS: &[&str] = &[
    "BROWSER",
    "DBUS_SESSION_BUS_ADDRESS",
    "DESKTOP_SESSION",
    "DISPLAY",
    "SSH_AUTH_SOCK",
    "SSH_CONNECTION",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_CURRENT_DESKTOP",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_TYPE",
];

pub const WSL_SESSION_ENV_KEYS: &[&str] = &[
    "WSL_DISTRO_NAME",
    "WSL_INTEROP",
    "WSLENV",
    "WT_PROFILE_ID",
    "WT_SESSION",
];

pub const USER_SESSION_TRANSPORT_ENV_KEYS: &[&str] = &[
    // NETWORK_PROXY_ENV_KEYS
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "WS_PROXY",
    "WSS_PROXY",
    "ws_proxy",
    "wss_proxy",
    "NPM_CONFIG_PROXY",
    "NPM_CONFIG_HTTPS_PROXY",
    "NPM_CONFIG_NO_PROXY",
    "npm_config_proxy",
    "npm_config_https_proxy",
    "npm_config_no_proxy",
    "YARN_PROXY",
    "YARN_HTTPS_PROXY",
    "YARN_NO_PROXY",
    "yarn_proxy",
    "yarn_https_proxy",
    "yarn_no_proxy",
    "BUNDLE_HTTPS_PROXY",
    "BUNDLE_NO_PROXY",
    "bundle_https_proxy",
    "bundle_no_proxy",
    // TRUST_STORE_ENV_KEYS
    "CODEX_CA_CERTIFICATE",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "NPM_CONFIG_CAFILE",
    "npm_config_cafile",
    // DESKTOP_SESSION_ENV_KEYS
    "BROWSER",
    "DBUS_SESSION_BUS_ADDRESS",
    "DESKTOP_SESSION",
    "DISPLAY",
    "SSH_AUTH_SOCK",
    "SSH_CONNECTION",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_CURRENT_DESKTOP",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_TYPE",
    // WSL_SESSION_ENV_KEYS
    "WSL_DISTRO_NAME",
    "WSL_INTEROP",
    "WSLENV",
    "WT_PROFILE_ID",
    "WT_SESSION",
];

pub fn user_session_transport_env(
    environ: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let keys: HashSet<&str> = USER_SESSION_TRANSPORT_ENV_KEYS.iter().copied().collect();
    let source: HashMap<String, String> = match environ {
        Some(e) => e.clone(),
        None => std::env::vars().collect(),
    };

    source
        .into_iter()
        .filter(|(key, value)| keys.contains(key.as_str()) && !value.trim().is_empty())
        .collect()
}
