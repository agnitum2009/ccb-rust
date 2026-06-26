from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from ccbd.app_runtime.lifecycle import _runtime_accelerator_startup_actions
from runtime_accelerator.lifecycle import (
    RuntimeAcceleratorHandle,
    maybe_start_runtime_accelerator,
    stop_runtime_accelerator,
)


class FakeProcess:
    def __init__(self) -> None:
        self.returncode: int | None = None
        self.terminated = False
        self.killed = False

    def poll(self):
        return self.returncode

    def terminate(self) -> None:
        self.terminated = True
        self.returncode = 0

    def kill(self) -> None:
        self.killed = True
        self.returncode = -9

    def wait(self, timeout=None):
        return self.returncode


def test_runtime_accelerator_lifecycle_can_be_disabled(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "0")

    handle = maybe_start_runtime_accelerator(tmp_path)

    assert handle.enabled is False
    assert handle.process is None
    assert handle.error == ""


def test_runtime_accelerator_lifecycle_is_default_on_with_fallback(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.delenv("CCB_RUNTIME_ACCELERATOR_CODEX", raising=False)
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_BIN", str(tmp_path / "missing-bin"))

    handle = maybe_start_runtime_accelerator(tmp_path)

    assert handle.enabled is True
    assert handle.process is None
    assert handle.error == "missing_binary"


def test_runtime_accelerator_missing_binary_keeps_fallback(monkeypatch, tmp_path: Path) -> None:
    socket_path = tmp_path / "manual.sock"
    socket_path.write_text("", encoding="utf-8")
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "1")
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_BIN", str(tmp_path / "missing-bin"))
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_SOCKET", str(socket_path))

    handle = maybe_start_runtime_accelerator(tmp_path)
    stop_runtime_accelerator(handle)

    assert handle.enabled is True
    assert handle.process is None
    assert handle.error == "missing_binary"
    assert socket_path.exists()


def test_runtime_accelerator_start_and_stop_are_owned_by_handle(monkeypatch, tmp_path: Path) -> None:
    fake_process = FakeProcess()
    socket_path = tmp_path / ".ccb" / "runtime-accelerator" / "accelerator.sock"
    calls: dict[str, object] = {}

    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_CODEX", "1")
    monkeypatch.setenv("CCB_RUNTIME_ACCELERATOR_SOCKET", str(socket_path))
    monkeypatch.setattr("runtime_accelerator.lifecycle.accelerator_binary", lambda: "/bin/fake")
    monkeypatch.setattr("runtime_accelerator.lifecycle.wait_for_socket", lambda *args, **kwargs: True)

    def fake_popen(args, **kwargs):
        calls["args"] = args
        calls["kwargs"] = kwargs
        return fake_process

    monkeypatch.setattr("runtime_accelerator.lifecycle.subprocess.Popen", fake_popen)

    handle = maybe_start_runtime_accelerator(tmp_path)

    assert handle.started is True
    assert calls["args"] == ["/bin/fake", "serve", "--socket", str(socket_path)]
    socket_path.parent.mkdir(parents=True, exist_ok=True)
    socket_path.write_text("", encoding="utf-8")

    stop_runtime_accelerator(handle)

    assert fake_process.terminated is True
    assert fake_process.killed is False
    assert not socket_path.exists()


def test_ccbd_startup_actions_record_started_or_fallback() -> None:
    started = SimpleNamespace(runtime_accelerator=RuntimeAcceleratorHandle(True, None, process=FakeProcess()))
    fallback = SimpleNamespace(runtime_accelerator=RuntimeAcceleratorHandle(True, None, error="missing_binary"))
    disabled = SimpleNamespace(runtime_accelerator=RuntimeAcceleratorHandle(False, None))

    assert _runtime_accelerator_startup_actions(started) == ["start_runtime_accelerator"]
    assert _runtime_accelerator_startup_actions(fallback) == ["runtime_accelerator_fallback:missing_binary"]
    assert _runtime_accelerator_startup_actions(disabled) == []
