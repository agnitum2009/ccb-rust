from __future__ import annotations

import json
import os
import selectors
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from provider_core.runtime_specs import provider_marker_prefix

from .runtime_state import BridgeRuntimeState


class PersistentFifoReader:
    """Keep the bridge FIFO read end open so idle waits do not drop the reader."""

    def __init__(self, fifo_path: Path):
        self._path = Path(fifo_path)
        self._read_fd: int | None = None
        self._keepalive_fd: int | None = None
        self._selector: selectors.BaseSelector | None = None
        self._buffer = b''

    def _ensure_open(self) -> bool:
        if self._read_fd is not None:
            return True
        if not self._path.exists():
            return False
        try:
            read_fd = os.open(str(self._path), os.O_RDONLY | os.O_NONBLOCK)
            keepalive_fd = os.open(str(self._path), os.O_WRONLY | os.O_NONBLOCK)
        except OSError:
            try:
                os.close(read_fd)  # type: ignore[possibly-undefined]
            except Exception:
                pass
            return False
        selector: selectors.BaseSelector = selectors.DefaultSelector()
        try:
            selector.register(read_fd, selectors.EVENT_READ)
        except Exception:
            selector.close()
            selector = selectors.SelectSelector()
            try:
                selector.register(read_fd, selectors.EVENT_READ)
            except Exception:
                selector.close()
                os.close(read_fd)
                os.close(keepalive_fd)
                return False
        self._read_fd = read_fd
        self._keepalive_fd = keepalive_fd
        self._selector = selector
        return True

    def _pop_line(self) -> str | None:
        if b'\n' not in self._buffer:
            return None
        raw, self._buffer = self._buffer.split(b'\n', 1)
        return raw.decode('utf-8', errors='replace')

    def read_line(self, timeout: float) -> str | None:
        line = self._pop_line()
        if line is not None:
            return line
        if not self._ensure_open():
            if timeout > 0:
                time.sleep(min(timeout, 0.2))
            return None
        assert self._selector is not None and self._read_fd is not None
        if not self._selector.select(timeout):
            return None
        try:
            chunk = os.read(self._read_fd, 65536)
        except BlockingIOError:
            return None
        except OSError:
            self.close()
            return None
        if chunk:
            self._buffer += chunk
        return self._pop_line()

    def close(self) -> None:
        if self._selector is not None:
            try:
                self._selector.close()
            except Exception:
                pass
            self._selector = None
        for attr in ('_read_fd', '_keepalive_fd'):
            fd = getattr(self, attr)
            if fd is not None:
                try:
                    os.close(fd)
                except OSError:
                    pass
                setattr(self, attr, None)


def _parse_request_line(line: str) -> dict[str, Any] | None:
    if not line.strip():
        return None
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return None


def read_request(state: BridgeRuntimeState, *, timeout: float = 0.0) -> dict[str, Any] | None:
    reader = getattr(state, 'fifo_reader', None)
    if reader is not None:
        line = reader.read_line(timeout)
        if line is None:
            return None
        return _parse_request_line(line)
    if not state.paths.input_fifo.exists():
        return None
    try:
        with state.paths.input_fifo.open('r', encoding='utf-8') as fifo:
            line = fifo.readline()
            if not line:
                return None
            return _parse_request_line(line)
    except OSError:
        return None


def process_request(
    state: BridgeRuntimeState,
    payload: dict[str, Any],
    *,
    log_console_fn,
) -> None:
    content = payload.get('content', '')
    marker = payload.get('marker') or generate_marker()
    timestamp = timestamp_now()
    log_bridge(
        state,
        json.dumps({'marker': marker, 'question': content, 'time': timestamp}, ensure_ascii=False),
    )
    append_history(state, 'claude', content, marker, log_console_fn=log_console_fn)

    try:
        state.codex_session.send(content)
    except Exception as exc:
        message = f'Failed to send to Codex: {exc}'
        append_history(state, 'codex', message, marker, log_console_fn=log_console_fn)
        log_console_fn(message)


def append_history(
    state: BridgeRuntimeState,
    role: str,
    content: str,
    marker: str,
    *,
    log_console_fn,
) -> None:
    entry = {
        'timestamp': datetime.now(timezone.utc).isoformat(),
        'role': role,
        'marker': marker,
        'content': content,
    }
    try:
        with state.paths.history_file.open('a', encoding='utf-8') as handle:
            json.dump(entry, handle, ensure_ascii=False)
            handle.write('\n')
    except Exception as exc:
        log_console_fn(f'Failed to write history: {exc}')


def log_bridge(state: BridgeRuntimeState, message: str) -> None:
    try:
        with state.paths.bridge_log.open('a', encoding='utf-8') as handle:
            handle.write(f'{timestamp_now()} {message}\n')
    except Exception:
        pass


def timestamp_now() -> str:
    return datetime.now().strftime('%Y-%m-%d %H:%M:%S')


def generate_marker() -> str:
    return f"{provider_marker_prefix('codex')}-{int(time.time())}-{os.getpid()}"


__all__ = [
    'PersistentFifoReader',
    'append_history',
    'generate_marker',
    'log_bridge',
    'process_request',
    'read_request',
    'timestamp_now',
]
