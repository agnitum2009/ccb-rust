from __future__ import annotations

import fcntl
import json
import os
from pathlib import Path
from typing import Any


class MaintenanceHeartbeatLockBusy(RuntimeError):
    pass


class MaintenanceHeartbeatLock:
    def __init__(self, path: Path, *, payload: dict[str, Any]) -> None:
        self._path = Path(path)
        self._payload = dict(payload)
        self._handle = None

    def __enter__(self) -> 'MaintenanceHeartbeatLock':
        self._path.parent.mkdir(parents=True, exist_ok=True)
        handle = self._path.open('a+', encoding='utf-8')
        try:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            handle.close()
            raise MaintenanceHeartbeatLockBusy('maintenance heartbeat tick is already running') from exc
        self._handle = handle
        self._write_state({'held': True, **self._payload})
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        handle = self._handle
        if handle is None:
            return
        try:
            release_payload = dict(self._payload)
            released_at = release_payload.pop('released_at', None)
            self._write_state({'held': False, **release_payload, 'released_at': released_at})
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
        finally:
            handle.close()
            self._handle = None

    def _write_state(self, payload: dict[str, Any]) -> None:
        handle = self._handle
        if handle is None:
            return
        handle.seek(0)
        handle.truncate(0)
        handle.write(json.dumps(payload, ensure_ascii=False, indent=2) + '\n')
        handle.flush()
        os.fsync(handle.fileno())


__all__ = ['MaintenanceHeartbeatLock', 'MaintenanceHeartbeatLockBusy']
