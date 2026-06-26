from __future__ import annotations

import json
from pathlib import Path

from provider_backends.codex.bridge_runtime.service import DualBridge


class _FakeTracker:
    def __init__(self) -> None:
        self.started = 0
        self.stopped = 0

    def start(self) -> None:
        self.started += 1

    def stop(self) -> None:
        self.stopped += 1


class _FakeSession:
    def __init__(self) -> None:
        self.sent: list[str] = []

    def send(self, content: str) -> None:
        self.sent.append(content)


def test_dual_bridge_processes_request_and_records_history(tmp_path: Path, monkeypatch) -> None:
    tracker = _FakeTracker()
    session = _FakeSession()
    monkeypatch.setenv('CODEX_TMUX_SESSION', '%7')
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.CodexBindingTracker',
        lambda runtime_dir: tracker,
    )
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.TerminalCodexSession',
        lambda pane_id: session,
    )
    bridge = DualBridge(tmp_path / 'runtime')
    bridge.input_fifo.write_text(json.dumps({'content': 'hello', 'marker': 'mk-1'}) + '\n', encoding='utf-8')

    payload = bridge._read_request()

    assert payload == {'content': 'hello', 'marker': 'mk-1'}
    bridge._process_request(payload)
    assert session.sent == ['hello']
    history = bridge.history_file.read_text(encoding='utf-8')
    assert '"role": "claude"' in history
    assert '"marker": "mk-1"' in history
    assert 'hello' in history
    assert 'mk-1' in bridge.bridge_log.read_text(encoding='utf-8')


def test_dual_bridge_handles_session_send_failure(tmp_path: Path, monkeypatch) -> None:
    tracker = _FakeTracker()

    class _FailingSession:
        def send(self, content: str) -> None:
            raise RuntimeError(f'boom:{content}')

    monkeypatch.setenv('CODEX_TMUX_SESSION', '%8')
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.CodexBindingTracker',
        lambda runtime_dir: tracker,
    )
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.TerminalCodexSession',
        lambda pane_id: _FailingSession(),
    )
    bridge = DualBridge(tmp_path / 'runtime')

    bridge._process_request({'content': 'fail-me', 'marker': 'mk-2'})

    history_lines = bridge.history_file.read_text(encoding='utf-8').splitlines()
    assert len(history_lines) == 2
    first = json.loads(history_lines[0])
    second = json.loads(history_lines[1])
    assert first['role'] == 'claude'
    assert second['role'] == 'codex'
    assert second['content'] == 'Failed to send to Codex: boom:fail-me'


def test_dual_bridge_defaults_to_event_wait_instead_of_hot_idle_poll(tmp_path: Path, monkeypatch) -> None:
    tracker = _FakeTracker()
    session = _FakeSession()
    observed_timeouts: list[float] = []
    monkeypatch.delenv('CCB_BRIDGE_IDLE_SLEEP', raising=False)
    monkeypatch.setenv('CODEX_TMUX_SESSION', '%9')
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.CodexBindingTracker',
        lambda runtime_dir: tracker,
    )
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.TerminalCodexSession',
        lambda pane_id: session,
    )
    bridge = DualBridge(tmp_path / 'runtime')

    def fake_read_request(*, timeout: float = 0.0):
        observed_timeouts.append(timeout)
        bridge._running = False
        return None

    monkeypatch.setattr(bridge, '_read_request', fake_read_request)

    assert bridge.run() == 0

    assert observed_timeouts == [1.0]
    assert tracker.started == 1
    assert tracker.stopped >= 1


def test_dual_bridge_respects_explicit_idle_sleep_override(tmp_path: Path, monkeypatch) -> None:
    tracker = _FakeTracker()
    session = _FakeSession()
    observed_timeouts: list[float] = []
    monkeypatch.setenv('CCB_BRIDGE_IDLE_SLEEP', '0.05')
    monkeypatch.setenv('CODEX_TMUX_SESSION', '%10')
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.CodexBindingTracker',
        lambda runtime_dir: tracker,
    )
    monkeypatch.setattr(
        'provider_backends.codex.bridge_runtime.runtime_state.TerminalCodexSession',
        lambda pane_id: session,
    )
    bridge = DualBridge(tmp_path / 'runtime')

    def fake_read_request(*, timeout: float = 0.0):
        observed_timeouts.append(timeout)
        bridge._running = False
        return None

    monkeypatch.setattr(bridge, '_read_request', fake_read_request)

    assert bridge.run() == 0

    assert observed_timeouts == [0.05]
