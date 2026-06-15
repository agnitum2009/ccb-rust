from __future__ import annotations

from provider_core.runtime_specs import (
    AGY_CLIENT_SPEC,
    AGY_RUNTIME_SPEC,
    CLIENT_SPECS_BY_PROVIDER,
    CODEX_CLIENT_SPEC,
    CODEX_RUNTIME_SPEC,
    DEEPSEEK_CLIENT_SPEC,
    DEEPSEEK_RUNTIME_SPEC,
    KIMI_CLIENT_SPEC,
    KIMI_RUNTIME_SPEC,
    RUNTIME_SPECS_BY_PROVIDER,
    provider_env_name,
    provider_marker_prefix,
)


def test_runtime_specs_use_provider_native_names() -> None:
    assert CODEX_RUNTIME_SPEC.provider_key == "codex"
    assert CODEX_RUNTIME_SPEC.service_name == "codex"
    assert CODEX_RUNTIME_SPEC.state_file_name == "codex-runtime.json"
    assert CODEX_RUNTIME_SPEC.log_file_name == "codex-runtime.log"
    assert CODEX_RUNTIME_SPEC.idle_timeout_env == "CCB_CODEX_RUNTIME_IDLE_TIMEOUT_S"
    assert CODEX_RUNTIME_SPEC.lock_name == "codex-runtime"

    assert CODEX_CLIENT_SPEC.provider_key == "codex"
    assert CODEX_CLIENT_SPEC.enabled_env == "CCB_CODEX"
    assert CODEX_CLIENT_SPEC.autostart_env == "CCB_CODEX_AUTOSTART"
    assert CODEX_CLIENT_SPEC.state_file_env == "CCB_CODEX_STATE_FILE"
    assert CODEX_CLIENT_SPEC.session_filename == ".codex-session"
    assert provider_env_name("claude", "PANE_CHECK_INTERVAL") == "CCB_CLAUDE_PANE_CHECK_INTERVAL"
    assert provider_env_name("codebuddy", "REBIND_TAIL_BYTES") == "CCB_CODEBUDDY_REBIND_TAIL_BYTES"
    assert provider_marker_prefix("opencode") == "opencode"
    assert AGY_RUNTIME_SPEC.provider_key == "agy"
    assert AGY_RUNTIME_SPEC.service_name == "agy"
    assert AGY_RUNTIME_SPEC.state_file_name == "agy-runtime.json"
    assert AGY_RUNTIME_SPEC.idle_timeout_env == "CCB_AGY_RUNTIME_IDLE_TIMEOUT_S"
    assert AGY_CLIENT_SPEC.provider_key == "agy"
    assert AGY_CLIENT_SPEC.state_file_env == "CCB_AGY_STATE_FILE"
    assert AGY_CLIENT_SPEC.session_filename == ".agy-session"
    assert RUNTIME_SPECS_BY_PROVIDER["agy"] is AGY_RUNTIME_SPEC
    assert CLIENT_SPECS_BY_PROVIDER["agy"] is AGY_CLIENT_SPEC
    assert KIMI_RUNTIME_SPEC.provider_key == "kimi"
    assert KIMI_RUNTIME_SPEC.idle_timeout_env == "CCB_KIMI_RUNTIME_IDLE_TIMEOUT_S"
    assert KIMI_CLIENT_SPEC.session_filename == ".kimi-session"
    assert DEEPSEEK_RUNTIME_SPEC.provider_key == "deepseek"
    assert DEEPSEEK_RUNTIME_SPEC.idle_timeout_env == "CCB_DEEPSEEK_RUNTIME_IDLE_TIMEOUT_S"
    assert DEEPSEEK_CLIENT_SPEC.session_filename == ".deepseek-session"
    assert RUNTIME_SPECS_BY_PROVIDER["kimi"] is KIMI_RUNTIME_SPEC
    assert CLIENT_SPECS_BY_PROVIDER["deepseek"] is DEEPSEEK_CLIENT_SPEC
