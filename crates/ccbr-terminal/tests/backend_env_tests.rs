//! Mirrors Python `test/test_terminal_runtime_backend_env.py`.

use ccbr_terminal::backend_env::{apply_backend_env_with, get_backend_env};

#[test]
fn test_get_backend_env_prefers_explicit_env() {
    std::env::set_var("CCBR_BACKEND_ENV", "wsl");
    assert_eq!(get_backend_env(), Some("wsl".to_string()));
    std::env::remove_var("CCBR_BACKEND_ENV");
}

#[test]
fn test_apply_backend_env_uses_existing_wsl_paths() {
    std::env::set_var("CCBR_BACKEND_ENV", "wsl");
    std::env::remove_var("CODEX_SESSION_ROOT");
    std::env::remove_var("GEMINI_ROOT");

    apply_backend_env_with(
        true,
        || ("Ubuntu".to_string(), "/home/demo".to_string()),
        |path| path.ends_with(r"\.codex\sessions"),
    );

    assert!(std::env::var("CODEX_SESSION_ROOT")
        .unwrap()
        .ends_with(r".codex\sessions"));
    assert!(std::env::var("GEMINI_ROOT")
        .unwrap()
        .ends_with(r".gemini\tmp"));

    std::env::remove_var("CCBR_BACKEND_ENV");
    std::env::remove_var("CODEX_SESSION_ROOT");
    std::env::remove_var("GEMINI_ROOT");
}

#[test]
fn test_apply_backend_env_falls_back_to_localhost_prefix() {
    std::env::set_var("CCBR_BACKEND_ENV", "wsl");
    std::env::remove_var("CODEX_SESSION_ROOT");
    std::env::remove_var("GEMINI_ROOT");

    apply_backend_env_with(
        true,
        || ("Ubuntu".to_string(), "/root".to_string()),
        |_path| false,
    );

    assert!(std::env::var("CODEX_SESSION_ROOT")
        .unwrap()
        .starts_with(r"\\wsl.localhost\Ubuntu"));
    assert!(std::env::var("GEMINI_ROOT")
        .unwrap()
        .starts_with(r"\\wsl.localhost\Ubuntu"));

    std::env::remove_var("CCBR_BACKEND_ENV");
    std::env::remove_var("CODEX_SESSION_ROOT");
    std::env::remove_var("GEMINI_ROOT");
}
