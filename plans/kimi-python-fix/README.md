# Kimi Python Provider Fix Patch

This folder preserves a Python-release patch for the Kimi CCB channel fixes that
were validated in the installed runtime before the Rust migration port.

## Artifact

- `kimi-python-provider-fix.patch`

## Source

Generated from the current installed Python release under:

- `/root/.local/share/codex-dual/lib/provider_backends/kimi/execution.py`
- `/root/.local/share/codex-dual/lib/provider_backends/kimi/launcher.py`
- `/root/.local/share/codex-dual/test/test_native_cli_completion.py`
- `/root/.local/share/codex-dual/test/test_native_cli_providers.py`

## Scope

The patch captures:

- Kimi pane fallback reply extraction.
- K2.7 input readiness detection.
- Non-answer/progress-only filtering.
- Kimi context projection pointer support.
- Regression tests for the above behavior.

The patch is intentionally larger than the normal documentation budget because
it is a generated source patch, not a design document.

## Boundary

Use this only for Python CCB releases. The canonical migration target remains
the Rust workspace under `rust/`.
