from __future__ import annotations

import shlex
from pathlib import Path

from runtime_env.user_session import user_session_transport_env


def caller_context_env(*, actor: str, runtime_dir: Path, launch_session_id: str) -> dict[str, str]:
    return {
        'CCB_CALLER_ACTOR': str(actor or '').strip(),
        'CCB_CALLER_RUNTIME_DIR': str(runtime_dir),
        'CCB_SESSION_ID': str(launch_session_id or '').strip(),
    }


def provider_user_session_env() -> dict[str, str]:
    return user_session_transport_env()


def export_env_clause(env_map: dict[str, str]) -> str:
    rendered = ' '.join(
        f'{key}={shlex.quote(str(value))}'
        for key, value in sorted(env_map.items())
        if str(value).strip()
    )
    if not rendered:
        return ''
    return f'export {rendered}'


def join_env_prefix(*clauses: str) -> str:
    return '; '.join(str(clause).strip() for clause in clauses if str(clause).strip())


__all__ = ['caller_context_env', 'export_env_clause', 'join_env_prefix', 'provider_user_session_env']
