use ccbr_types::control_plane::control_plane_env;
use serial_test::serial;
use std::collections::HashMap;

#[test]
#[serial]
fn test_control_plane_env_keeps_provider_api_env() {
    std::env::set_var("OPENAI_API_KEY", "openai-key");
    std::env::set_var("OPENAI_BASE_URL", "https://api.example.test/v1");
    std::env::set_var("ANTHROPIC_API_KEY", "anthropic-key");
    std::env::set_var("GEMINI_API_KEY", "gemini-key");
    std::env::set_var("GEMINI_MODEL", "gemini-3.1-pro-preview");
    std::env::set_var("GOOGLE_GEMINI_BASE_URL", "https://chatapi.onechats.ai");

    let env = control_plane_env(None);

    assert_eq!(env.get("OPENAI_API_KEY").unwrap(), "openai-key");
    assert_eq!(
        env.get("OPENAI_BASE_URL").unwrap(),
        "https://api.example.test/v1"
    );
    assert_eq!(env.get("ANTHROPIC_API_KEY").unwrap(), "anthropic-key");
    assert_eq!(env.get("GEMINI_API_KEY").unwrap(), "gemini-key");
    assert_eq!(env.get("GEMINI_MODEL").unwrap(), "gemini-3.1-pro-preview");
    assert_eq!(
        env.get("GOOGLE_GEMINI_BASE_URL").unwrap(),
        "https://chatapi.onechats.ai"
    );
}

#[test]
#[serial]
fn test_control_plane_env_keeps_claude_keychain_override() {
    std::env::set_var(
        "CCBR_KEYCHAIN_SERVICE_OVERRIDE",
        "Claude Code-credentials-account-a",
    );

    let env = control_plane_env(None);

    assert_eq!(
        env.get("CCBR_KEYCHAIN_SERVICE_OVERRIDE").unwrap(),
        "Claude Code-credentials-account-a"
    );
}

#[test]
#[serial]
fn test_control_plane_env_keeps_user_session_transport_for_cmd_shell() {
    std::env::set_var("DISPLAY", ":0");
    std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus");
    std::env::set_var("XAUTHORITY", "/tmp/.Xauthority");
    std::env::set_var("SSH_AUTH_SOCK", "/tmp/ssh-agent.sock");

    let env = control_plane_env(None);

    assert_eq!(env.get("DISPLAY").unwrap(), ":0");
    assert_eq!(env.get("WAYLAND_DISPLAY").unwrap(), "wayland-0");
    assert_eq!(
        env.get("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        "unix:path=/run/user/1000/bus"
    );
    assert_eq!(env.get("XAUTHORITY").unwrap(), "/tmp/.Xauthority");
    assert_eq!(env.get("SSH_AUTH_SOCK").unwrap(), "/tmp/ssh-agent.sock");
}

#[test]
#[serial]
fn test_control_plane_env_keeps_network_transport_without_provider_authority() {
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:7890");
    std::env::set_var("NO_PROXY", "localhost,127.0.0.1");
    std::env::set_var("CODEX_CA_CERTIFICATE", "/tmp/codex-ca.pem");
    std::env::set_var("SSL_CERT_FILE", "/tmp/ca.pem");
    std::env::set_var("WSL_INTEROP", "/run/WSL/1234_interop");
    std::env::set_var("WSL_DISTRO_NAME", "Ubuntu-22.04");
    std::env::set_var("CODEX_HOME", "/tmp/global-codex-home");
    std::env::set_var("CODEX_SESSION_ROOT", "/tmp/global-codex-sessions");
    std::env::set_var("GEMINI_ROOT", "/tmp/global-gemini-root");
    std::env::set_var("CLAUDE_PROJECTS_ROOT", "/tmp/global-claude-projects");
    std::env::set_var("CCBR_SESSION_ID", "stale-session");
    std::env::set_var("CCBR_CALLER_ACTOR", "stale-agent");

    let env = control_plane_env(None);

    assert_eq!(env.get("HTTPS_PROXY").unwrap(), "http://127.0.0.1:7890");
    assert_eq!(env.get("NO_PROXY").unwrap(), "localhost,127.0.0.1");
    assert_eq!(
        env.get("CODEX_CA_CERTIFICATE").unwrap(),
        "/tmp/codex-ca.pem"
    );
    assert_eq!(env.get("SSL_CERT_FILE").unwrap(), "/tmp/ca.pem");
    assert_eq!(env.get("WSL_INTEROP").unwrap(), "/run/WSL/1234_interop");
    assert_eq!(env.get("WSL_DISTRO_NAME").unwrap(), "Ubuntu-22.04");
    assert!(!env.contains_key("CODEX_HOME"));
    assert!(!env.contains_key("CODEX_SESSION_ROOT"));
    assert!(!env.contains_key("GEMINI_ROOT"));
    assert!(!env.contains_key("CLAUDE_PROJECTS_ROOT"));
    assert!(!env.contains_key("CCBR_SESSION_ID"));
    assert!(!env.contains_key("CCBR_CALLER_ACTOR"));
}

#[test]
#[serial]
fn test_control_plane_env_drops_outer_tmux_authority() {
    std::env::set_var("TMUX", "/tmp/tmux-1000/default,123,0");
    std::env::set_var("TMUX_PANE", "%77");
    std::env::set_var("CCBR_TMUX_SOCKET", "outer");
    std::env::set_var("CCBR_TMUX_SOCKET_PATH", "/tmp/outer.sock");

    let env = control_plane_env(None);

    assert!(!env.contains_key("TMUX"));
    assert!(!env.contains_key("TMUX_PANE"));
    assert!(!env.contains_key("CCBR_TMUX_SOCKET"));
    assert!(!env.contains_key("CCBR_TMUX_SOCKET_PATH"));
}

#[test]
#[serial]
fn test_control_plane_env_drops_outer_pythonpath() {
    std::env::set_var("PYTHONPATH", "/stable/ccb/lib:/other");
    std::env::set_var("PYTHONUNBUFFERED", "1");

    let env = control_plane_env(None);

    assert!(!env.contains_key("PYTHONPATH"));
    assert_eq!(env.get("PYTHONUNBUFFERED").unwrap(), "1");
}

#[test]
fn test_control_plane_env_extra_overrides_and_removes() {
    let mut extra = HashMap::new();
    extra.insert("CUSTOM_KEY".to_string(), Some("custom-value".to_string()));
    extra.insert("PYTHONUNBUFFERED".to_string(), None);

    // Ensure PYTHONUNBUFFERED is not set globally for this test.
    std::env::remove_var("PYTHONUNBUFFERED");

    let env = control_plane_env(Some(&extra));

    assert_eq!(env.get("CUSTOM_KEY").unwrap(), "custom-value");
    assert!(!env.contains_key("PYTHONUNBUFFERED"));
}
