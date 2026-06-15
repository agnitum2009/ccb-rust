from __future__ import annotations

from provider_backends.codex.execution_runtime.readiness import looks_ready, looks_unusable, wait_for_runtime_ready


def test_codex_looks_ready_rejects_loading_banner() -> None:
    assert not looks_ready('>_ OpenAI Codex\nmodel: loading /model to change\n› Implement {feature}')


def test_codex_looks_ready_accepts_ready_banner() -> None:
    assert looks_ready('>_ OpenAI Codex\nmodel: gpt-5.5 xhigh /model to change\n› Implement {feature}')


def test_codex_looks_ready_rejects_shutdown_text() -> None:
    text = '>_ OpenAI Codex\nShutting down...\nPane is dead'

    assert looks_unusable(text)
    assert not looks_ready(text)


def test_codex_unusable_marker_ignores_legitimate_conversation_text() -> None:
    text = (
        '>_ OpenAI Codex\n'
        'model: gpt-5.5 xhigh /model to change\n'
        'The user asked how to handle shutting down a Kubernetes cluster.\n'
        '› Implement {feature}'
    )

    assert not looks_unusable(text)
    assert looks_ready(text)


def test_codex_wait_for_runtime_ready_waits_for_stable_ready_prompt(monkeypatch) -> None:
    class _Backend:
        def __init__(self) -> None:
            self.calls = 0

        def get_pane_content(self, pane_id: str, *, lines: int = 120) -> str:
            del pane_id, lines
            self.calls += 1
            if self.calls == 1:
                return '>_ OpenAI Codex\nmodel: loading /model to change\n› Implement {feature}'
            return '>_ OpenAI Codex\nmodel: gpt-5.5 xhigh /model to change\n› Implement {feature}'

    backend = _Backend()
    time_values = iter([0.0, 0.0, 0.2, 0.2, 0.9, 0.9])

    monkeypatch.setattr('provider_backends.codex.execution_runtime.readiness.time.time', lambda: next(time_values))
    monkeypatch.setattr('provider_backends.codex.execution_runtime.readiness.time.sleep', lambda seconds: None)

    wait_for_runtime_ready(backend, '%1', timeout_s=3.0)

    assert backend.calls == 3
