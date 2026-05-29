from __future__ import annotations

import json
from time import monotonic

from ccbd.api_models import RpcRequest, RpcResponse

_REQUEST_READ_TIMEOUT_S = 0.5
_MAX_REQUEST_BYTES = 1024 * 1024


def handle_connection(server, conn) -> str | None:
    request = None
    after_response_action = None
    try:
        conn.settimeout(_REQUEST_READ_TIMEOUT_S)
        raw = _recv_request_line(conn)
        if not raw:
            return None
        message = json.loads(raw.split(b'\n', 1)[0].decode('utf-8'))
        request = RpcRequest.from_record(message)
        handler = server._handlers.get(request.op)
        if handler is None:
            response = RpcResponse.failure(f'unknown op: {request.op}')
        else:
            guard = getattr(server, '_request_guard', None)
            rejection = guard(request.op) if guard is not None else None
            if rejection:
                response = RpcResponse.failure(rejection)
            else:
                started = monotonic()
                try:
                    payload = handler(request.request)
                finally:
                    _record_handler_latency(server, request.op, max(0.0, monotonic() - started))
                if isinstance(payload, tuple) and len(payload) == 2:
                    payload, after_response_action = payload
                response = RpcResponse.success(payload)
    except Exception as exc:
        response = RpcResponse.failure(str(exc))
    try:
        conn.sendall((json.dumps(response.to_record(), ensure_ascii=False) + '\n').encode('utf-8'))
    except OSError:
        _queue_after_response_action(server, after_response_action)
        return getattr(request, 'op', None)
    _queue_after_response_action(server, after_response_action)
    return getattr(request, 'op', None)


def _recv_request_line(conn) -> bytes:
    raw = b''
    while b'\n' not in raw:
        chunk = conn.recv(65536)
        if not chunk:
            break
        raw += chunk
        if len(raw) > _MAX_REQUEST_BYTES:
            raise ValueError('ccbd request exceeds maximum size')
    return raw


def _queue_after_response_action(server, action) -> None:
    if action is None:
        return
    try:
        server.queue_after_response_action(action)
    except Exception:
        pass


def _record_handler_latency(server, op: str, duration: float) -> None:
    callback = getattr(server, '_record_handler_latency', None)
    if not callable(callback):
        return
    try:
        callback(op, duration)
    except Exception:
        pass


__all__ = ['handle_connection']
