from __future__ import annotations

from provider_core.contracts import ProviderBackend

CORE_PROVIDER_NAMES = ("codex", "claude", "gemini")
OPTIONAL_PROVIDER_NAMES = ("opencode", "droid", "agy", "kimi", "deepseek")


def build_builtin_backends(*, include_optional: bool = True) -> list[ProviderBackend]:
    from provider_backends.agy import build_backend as build_agy_backend
    from provider_backends.claude import build_backend as build_claude_backend
    from provider_backends.codex import build_backend as build_codex_backend
    from provider_backends.deepseek import build_backend as build_deepseek_backend
    from provider_backends.droid import build_backend as build_droid_backend
    from provider_backends.gemini import build_backend as build_gemini_backend
    from provider_backends.kimi import build_backend as build_kimi_backend
    from provider_backends.opencode import build_backend as build_opencode_backend

    backends = [
        build_codex_backend(),
        build_claude_backend(),
        build_gemini_backend(),
    ]
    if include_optional:
        backends.extend([
            build_opencode_backend(),
            build_droid_backend(),
            build_agy_backend(),
            build_kimi_backend(),
            build_deepseek_backend(),
        ])
    return backends


__all__ = ["CORE_PROVIDER_NAMES", "OPTIONAL_PROVIDER_NAMES", "build_builtin_backends"]
