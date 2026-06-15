from __future__ import annotations

from runtime_env.control_plane import control_plane_env


def test_control_plane_env_keeps_provider_api_env(monkeypatch) -> None:
    monkeypatch.setenv('OPENAI_API_KEY', 'openai-key')
    monkeypatch.setenv('OPENAI_BASE_URL', 'https://api.example.test/v1')
    monkeypatch.setenv('ANTHROPIC_API_KEY', 'anthropic-key')
    monkeypatch.setenv('GEMINI_API_KEY', 'gemini-key')
    monkeypatch.setenv('GEMINI_MODEL', 'gemini-3.1-pro-preview')
    monkeypatch.setenv('GOOGLE_GEMINI_BASE_URL', 'https://chatapi.onechats.ai')

    env = control_plane_env()

    assert env['OPENAI_API_KEY'] == 'openai-key'
    assert env['OPENAI_BASE_URL'] == 'https://api.example.test/v1'
    assert env['ANTHROPIC_API_KEY'] == 'anthropic-key'
    assert env['GEMINI_API_KEY'] == 'gemini-key'
    assert env['GEMINI_MODEL'] == 'gemini-3.1-pro-preview'
    assert env['GOOGLE_GEMINI_BASE_URL'] == 'https://chatapi.onechats.ai'


def test_control_plane_env_keeps_claude_keychain_override(monkeypatch) -> None:
    monkeypatch.setenv('CCB_KEYCHAIN_SERVICE_OVERRIDE', 'Claude Code-credentials-account-a')

    env = control_plane_env()

    assert env['CCB_KEYCHAIN_SERVICE_OVERRIDE'] == 'Claude Code-credentials-account-a'


def test_control_plane_env_keeps_user_session_transport_for_cmd_shell(monkeypatch) -> None:
    monkeypatch.setenv('DISPLAY', ':0')
    monkeypatch.setenv('WAYLAND_DISPLAY', 'wayland-0')
    monkeypatch.setenv('DBUS_SESSION_BUS_ADDRESS', 'unix:path=/run/user/1000/bus')
    monkeypatch.setenv('XAUTHORITY', '/tmp/.Xauthority')
    monkeypatch.setenv('SSH_AUTH_SOCK', '/tmp/ssh-agent.sock')

    env = control_plane_env()

    assert env['DISPLAY'] == ':0'
    assert env['WAYLAND_DISPLAY'] == 'wayland-0'
    assert env['DBUS_SESSION_BUS_ADDRESS'] == 'unix:path=/run/user/1000/bus'
    assert env['XAUTHORITY'] == '/tmp/.Xauthority'
    assert env['SSH_AUTH_SOCK'] == '/tmp/ssh-agent.sock'


def test_control_plane_env_keeps_network_transport_without_provider_authority(monkeypatch) -> None:
    monkeypatch.setenv('HTTPS_PROXY', 'http://127.0.0.1:7890')
    monkeypatch.setenv('NO_PROXY', 'localhost,127.0.0.1')
    monkeypatch.setenv('CODEX_CA_CERTIFICATE', '/tmp/codex-ca.pem')
    monkeypatch.setenv('SSL_CERT_FILE', '/tmp/ca.pem')
    monkeypatch.setenv('WSL_INTEROP', '/run/WSL/1234_interop')
    monkeypatch.setenv('WSL_DISTRO_NAME', 'Ubuntu-22.04')
    monkeypatch.setenv('CODEX_HOME', '/tmp/global-codex-home')
    monkeypatch.setenv('CODEX_SESSION_ROOT', '/tmp/global-codex-sessions')
    monkeypatch.setenv('GEMINI_ROOT', '/tmp/global-gemini-root')
    monkeypatch.setenv('CLAUDE_PROJECTS_ROOT', '/tmp/global-claude-projects')
    monkeypatch.setenv('CCB_SESSION_ID', 'stale-session')
    monkeypatch.setenv('CCB_CALLER_ACTOR', 'stale-agent')

    env = control_plane_env()

    assert env['HTTPS_PROXY'] == 'http://127.0.0.1:7890'
    assert env['NO_PROXY'] == 'localhost,127.0.0.1'
    assert env['CODEX_CA_CERTIFICATE'] == '/tmp/codex-ca.pem'
    assert env['SSL_CERT_FILE'] == '/tmp/ca.pem'
    assert env['WSL_INTEROP'] == '/run/WSL/1234_interop'
    assert env['WSL_DISTRO_NAME'] == 'Ubuntu-22.04'
    assert 'CODEX_HOME' not in env
    assert 'CODEX_SESSION_ROOT' not in env
    assert 'GEMINI_ROOT' not in env
    assert 'CLAUDE_PROJECTS_ROOT' not in env
    assert 'CCB_SESSION_ID' not in env
    assert 'CCB_CALLER_ACTOR' not in env


def test_control_plane_env_drops_outer_tmux_authority(monkeypatch) -> None:
    monkeypatch.setenv('TMUX', '/tmp/tmux-1000/default,123,0')
    monkeypatch.setenv('TMUX_PANE', '%77')
    monkeypatch.setenv('CCB_TMUX_SOCKET', 'outer')
    monkeypatch.setenv('CCB_TMUX_SOCKET_PATH', '/tmp/outer.sock')

    env = control_plane_env()

    assert 'TMUX' not in env
    assert 'TMUX_PANE' not in env
    assert 'CCB_TMUX_SOCKET' not in env
    assert 'CCB_TMUX_SOCKET_PATH' not in env


def test_control_plane_env_drops_outer_pythonpath(monkeypatch) -> None:
    monkeypatch.setenv('PYTHONPATH', '/stable/ccb/lib:/other')
    monkeypatch.setenv('PYTHONUNBUFFERED', '1')

    env = control_plane_env()

    assert 'PYTHONPATH' not in env
    assert env['PYTHONUNBUFFERED'] == '1'
