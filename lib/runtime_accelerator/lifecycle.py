from __future__ import annotations

import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from .config import (
    accelerator_binary,
    accelerator_socket_path,
    accelerator_startup_timeout_s,
    codex_accelerator_enabled,
)


@dataclass
class RuntimeAcceleratorHandle:
    enabled: bool
    socket_path: Path | None
    process: subprocess.Popen | None = None
    error: str = ""

    @property
    def started(self) -> bool:
        return self.process is not None and self.process.poll() is None


def maybe_start_runtime_accelerator(project_root: str | Path) -> RuntimeAcceleratorHandle:
    socket_path = accelerator_socket_path(project_root)
    if not codex_accelerator_enabled():
        return RuntimeAcceleratorHandle(enabled=False, socket_path=socket_path)
    if socket_path is None:
        return RuntimeAcceleratorHandle(enabled=True, socket_path=None, error="missing_socket_path")
    binary = accelerator_binary()
    if not binary:
        return RuntimeAcceleratorHandle(enabled=True, socket_path=socket_path, error="missing_binary")
    try:
        socket_path.parent.mkdir(parents=True, exist_ok=True)
        if socket_path.exists():
            socket_path.unlink()
        process = subprocess.Popen(
            [binary, "serve", "--socket", str(socket_path)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
    except Exception as exc:
        return RuntimeAcceleratorHandle(enabled=True, socket_path=socket_path, error=str(exc))
    if wait_for_socket(socket_path, process=process, timeout_s=accelerator_startup_timeout_s()):
        return RuntimeAcceleratorHandle(enabled=True, socket_path=socket_path, process=process)
    error = "startup_timeout" if process.poll() is None else f"exited:{process.returncode}"
    stop_runtime_accelerator(RuntimeAcceleratorHandle(enabled=True, socket_path=socket_path, process=process))
    return RuntimeAcceleratorHandle(enabled=True, socket_path=socket_path, error=error)


def wait_for_socket(socket_path: Path, *, process: subprocess.Popen, timeout_s: float) -> bool:
    deadline = time.monotonic() + max(0.0, timeout_s)
    while time.monotonic() <= deadline:
        if socket_path.exists():
            return True
        if process.poll() is not None:
            return False
        time.sleep(0.025)
    return socket_path.exists()


def stop_runtime_accelerator(handle: RuntimeAcceleratorHandle | None) -> None:
    if handle is None:
        return
    process = handle.process
    owns_socket = process is not None
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=1.0)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=1.0)
    socket_path = handle.socket_path
    if owns_socket and socket_path is not None:
        try:
            socket_path.unlink()
        except FileNotFoundError:
            pass
        except Exception:
            pass


__all__ = [
    "RuntimeAcceleratorHandle",
    "maybe_start_runtime_accelerator",
    "stop_runtime_accelerator",
    "wait_for_socket",
]
