from __future__ import annotations

import errno
from pathlib import Path
import json
import socket
import time

from ccbd.api_models import RpcRequest, RpcResponse

from .errors import CcbdClientError

_CONNECT_RETRY_INTERVAL_S = 0.05
_CONNECT_MAX_RETRIES = 2
_CONNECT_RETRY_ERRNOS = frozenset({
    errno.EAGAIN,
    errno.ECONNREFUSED,
    errno.ENOENT,
})


def connect_socket(socket_path: Path, *, timeout_s: float):
    if not hasattr(socket, 'AF_UNIX'):
        raise CcbdClientError('unix domain sockets are not supported on this platform')
    deadline = time.monotonic() + max(0.0, float(timeout_s))
    last_error: OSError | None = None
    for attempt in range(_CONNECT_MAX_RETRIES + 1):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(remaining)
        try:
            sock.connect(str(socket_path))
            return sock
        except OSError as exc:
            sock.close()
            last_error = exc
            if not _is_transient_connect_error(exc):
                break
            if attempt >= _CONNECT_MAX_RETRIES:
                break
            sleep_for = min(_CONNECT_RETRY_INTERVAL_S, max(0.0, deadline - time.monotonic()))
            if sleep_for <= 0:
                break
            time.sleep(sleep_for)
    if last_error is not None:
        raise CcbdClientError(str(last_error)) from last_error
    raise CcbdClientError('timed out')


def _is_transient_connect_error(exc: OSError) -> bool:
    return int(getattr(exc, 'errno', 0) or 0) in _CONNECT_RETRY_ERRNOS


def send_request(sock, request: RpcRequest) -> None:
    payload = json.dumps(request.to_record(), ensure_ascii=False) + '\n'
    sock.sendall(payload.encode('utf-8'))


def recv_response_line(sock) -> bytes:
    raw = b''
    while b'\n' not in raw:
        chunk = sock.recv(65536)
        if not chunk:
            break
        raw += chunk
    return raw


def decode_response(raw: bytes) -> RpcResponse:
    line = raw.split(b'\n', 1)[0].decode('utf-8')
    return RpcResponse.from_record(json.loads(line))


__all__ = ['connect_socket', 'decode_response', 'recv_response_line', 'send_request']
