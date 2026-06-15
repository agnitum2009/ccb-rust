from __future__ import annotations

from runtime_env.user_session import user_session_transport_env


def test_user_session_transport_env_selects_only_transport_keys() -> None:
    env = user_session_transport_env(
        {
            'HTTPS_PROXY': 'http://127.0.0.1:7890',
            'http_proxy': 'http://127.0.0.1:7891',
            'NO_PROXY': 'localhost,127.0.0.1',
            'CODEX_CA_CERTIFICATE': '/tmp/codex-ca.pem',
            'NODE_EXTRA_CA_CERTS': '/tmp/node-ca.pem',
            'WSL_INTEROP': '/run/WSL/1234_interop',
            'BROWSER': 'wslview',
            'CODEX_HOME': '/tmp/global-codex-home',
            'GEMINI_ROOT': '/tmp/global-gemini-root',
            'CLAUDE_PROJECTS_ROOT': '/tmp/global-claude-projects',
            'EMPTY_PROXY': '',
            'SSL_CERT_FILE': '',
        }
    )

    assert env == {
        'HTTPS_PROXY': 'http://127.0.0.1:7890',
        'http_proxy': 'http://127.0.0.1:7891',
        'NO_PROXY': 'localhost,127.0.0.1',
        'CODEX_CA_CERTIFICATE': '/tmp/codex-ca.pem',
        'NODE_EXTRA_CA_CERTS': '/tmp/node-ca.pem',
        'WSL_INTEROP': '/run/WSL/1234_interop',
        'BROWSER': 'wslview',
    }
