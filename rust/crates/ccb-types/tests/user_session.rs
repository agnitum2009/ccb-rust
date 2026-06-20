use ccb_types::user_session::user_session_transport_env;
use std::collections::HashMap;

#[test]
fn test_user_session_transport_env_selects_only_transport_keys() {
    let input = HashMap::from([
        (
            "HTTPS_PROXY".to_string(),
            "http://127.0.0.1:7890".to_string(),
        ),
        (
            "http_proxy".to_string(),
            "http://127.0.0.1:7891".to_string(),
        ),
        ("NO_PROXY".to_string(), "localhost,127.0.0.1".to_string()),
        (
            "CODEX_CA_CERTIFICATE".to_string(),
            "/tmp/codex-ca.pem".to_string(),
        ),
        (
            "NODE_EXTRA_CA_CERTS".to_string(),
            "/tmp/node-ca.pem".to_string(),
        ),
        (
            "WSL_INTEROP".to_string(),
            "/run/WSL/1234_interop".to_string(),
        ),
        ("BROWSER".to_string(), "wslview".to_string()),
        (
            "CODEX_HOME".to_string(),
            "/tmp/global-codex-home".to_string(),
        ),
        (
            "GEMINI_ROOT".to_string(),
            "/tmp/global-gemini-root".to_string(),
        ),
        (
            "CLAUDE_PROJECTS_ROOT".to_string(),
            "/tmp/global-claude-projects".to_string(),
        ),
        ("EMPTY_PROXY".to_string(), "".to_string()),
        ("SSL_CERT_FILE".to_string(), "".to_string()),
    ]);

    let env = user_session_transport_env(Some(&input));

    assert_eq!(env.get("HTTPS_PROXY").unwrap(), "http://127.0.0.1:7890");
    assert_eq!(env.get("http_proxy").unwrap(), "http://127.0.0.1:7891");
    assert_eq!(env.get("NO_PROXY").unwrap(), "localhost,127.0.0.1");
    assert_eq!(
        env.get("CODEX_CA_CERTIFICATE").unwrap(),
        "/tmp/codex-ca.pem"
    );
    assert_eq!(env.get("NODE_EXTRA_CA_CERTS").unwrap(), "/tmp/node-ca.pem");
    assert_eq!(env.get("WSL_INTEROP").unwrap(), "/run/WSL/1234_interop");
    assert_eq!(env.get("BROWSER").unwrap(), "wslview");
    assert!(!env.contains_key("CODEX_HOME"));
    assert!(!env.contains_key("GEMINI_ROOT"));
    assert!(!env.contains_key("CLAUDE_PROJECTS_ROOT"));
    assert!(!env.contains_key("EMPTY_PROXY"));
    assert!(!env.contains_key("SSL_CERT_FILE"));
}
