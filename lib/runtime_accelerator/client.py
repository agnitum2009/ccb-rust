from __future__ import annotations

import json
import socket
from pathlib import Path
from typing import Any, Callable, TypeVar

T = TypeVar("T")


class AcceleratorError(RuntimeError):
    pass


def default_socket_path(project_root: str | Path) -> Path:
    return (
        Path(project_root).expanduser().resolve()
        / ".ccb"
        / "runtime-accelerator"
        / "accelerator.sock"
    )


def call(
    socket_path: str | Path,
    method: str,
    params: dict[str, Any] | None = None,
    *,
    timeout_s: float = 0.2,
) -> dict[str, Any]:
    request = (
        json.dumps({"method": method, "params": params or {}}, ensure_ascii=False).encode("utf-8")
        + b"\n"
    )
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(timeout_s)
            client.connect(str(socket_path))
            client.sendall(request)
            raw = _readline(client)
    except OSError as exc:
        raise AcceleratorError(str(exc)) from exc
    try:
        response = json.loads(raw.decode("utf-8"))
    except json.JSONDecodeError as exc:
        raise AcceleratorError(f"invalid accelerator response: {exc}") from exc
    if not isinstance(response, dict):
        raise AcceleratorError("accelerator response is not an object")
    if not response.get("ok"):
        raise AcceleratorError(str(response.get("error") or "accelerator error"))
    result = response.get("result")
    return result if isinstance(result, dict) else {}


def call_or_fallback(
    socket_path: str | Path,
    method: str,
    params: dict[str, Any] | None,
    fallback: Callable[[], T],
    *,
    timeout_s: float = 0.2,
) -> dict[str, Any] | T:
    try:
        return call(socket_path, method, params, timeout_s=timeout_s)
    except AcceleratorError:
        return fallback()


def _readline(client: socket.socket) -> bytes:
    chunks: list[bytes] = []
    while True:
        chunk = client.recv(4096)
        if not chunk:
            break
        chunks.append(chunk)
        if b"\n" in chunk:
            break
    data = b"".join(chunks)
    return data.split(b"\n", 1)[0]
