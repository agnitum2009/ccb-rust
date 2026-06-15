from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
from typing import Iterable

from provider_backends.native_cli_support import clean_native_reply
from provider_core.source_home import current_provider_source_home


@dataclass(frozen=True)
class KimiTurnObservation:
    request_seen: bool
    completed: bool
    reply: str
    session_id: str | None
    session_path: str | None
    provider_turn_ref: str | None
    line_count: int
    native_started_at: object | None = None
    native_completed_at: object | None = None


def observe_kimi_turn(
    work_dir: Path,
    req_id: str,
    *,
    home_candidates: Iterable[Path] | None = None,
) -> KimiTurnObservation | None:
    if not req_id:
        return None
    observations: list[KimiTurnObservation] = []
    for wire_path in _wire_paths(work_dir, home_candidates=home_candidates):
        observed = _observe_wire_file(wire_path, req_id=req_id)
        if observed is not None:
            observations.append(observed)
    if not observations:
        return None
    completed = [item for item in observations if item.completed]
    if completed:
        return max(completed, key=_observation_sort_key)
    return max(observations, key=_observation_sort_key)


def kimi_project_hash(work_dir: Path) -> str:
    normalized = str(Path(work_dir).expanduser().resolve(strict=False))
    return hashlib.md5(normalized.encode("utf-8", "surrogateescape")).hexdigest()


def kimi_sessions_root(work_dir: Path, *, home: Path | None = None) -> Path:
    base = _kimi_home(home)
    return base / "sessions" / kimi_project_hash(work_dir)


def _wire_paths(work_dir: Path, *, home_candidates: Iterable[Path] | None) -> list[Path]:
    paths: list[Path] = []
    seen: set[Path] = set()
    for home in _candidate_homes(home_candidates):
        root = kimi_sessions_root(work_dir, home=home)
        if not root.is_dir():
            continue
        for path in root.glob("*/wire.jsonl"):
            try:
                resolved = path.resolve(strict=False)
            except Exception:
                resolved = path
            if resolved in seen:
                continue
            seen.add(resolved)
            paths.append(path)
    return sorted(paths, key=_path_mtime)


def _candidate_homes(home_candidates: Iterable[Path] | None) -> list[Path]:
    candidates: list[Path] = []
    explicit = os.environ.get("KIMI_HOME")
    if explicit:
        candidates.append(Path(explicit).expanduser())
    if home_candidates is not None:
        candidates.extend(Path(item).expanduser() for item in home_candidates)
    candidates.append(current_provider_source_home())
    candidates.append(Path.home().expanduser())
    unique: list[Path] = []
    seen: set[str] = set()
    for candidate in candidates:
        key = str(candidate)
        if key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def _kimi_home(home: Path | None) -> Path:
    if home is None:
        return current_provider_source_home() / ".kimi"
    if home.name == ".kimi":
        return home
    return home / ".kimi"


def _observe_wire_file(path: Path, *, req_id: str) -> KimiTurnObservation | None:
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None

    current: dict[str, object] | None = None
    latest: KimiTurnObservation | None = None
    for index, line in enumerate(lines, 1):
        try:
            event = json.loads(line)
        except Exception:
            continue
        if not isinstance(event, dict):
            continue
        event_type, payload, timestamp = _normalize_event(event)

        if event_type == "TurnBegin":
            if _payload_has_req_id(payload, req_id):
                current = {
                    "parts": [],
                    "started_at": timestamp,
                    "line": index,
                    "message_id": None,
                }
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=False,
                    completed_at=None,
                    line_count=index,
                )
            else:
                current = None
            continue

        if event_type in {"turn.prompt", "turn.started"}:
            if _value_has_req_id(payload, req_id):
                current = {
                    "parts": [],
                    "started_at": timestamp,
                    "line": index,
                    "message_id": _coerce_str(payload.get("turnId") or payload.get("turn_id")),
                }
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=False,
                    completed_at=None,
                    line_count=index,
                )
            continue

        if event_type == "context.append_message":
            message = payload.get("message")
            if not isinstance(message, dict):
                continue
            role = str(message.get("role") or "").strip().lower()
            content = _text_from_value(message.get("content"))
            if role == "user" and req_id in content:
                current = {
                    "parts": [],
                    "started_at": timestamp,
                    "line": index,
                    "message_id": None,
                }
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=False,
                    completed_at=None,
                    line_count=index,
                )
                continue
            if role == "user" and current is not None:
                current = None
                continue
            if current is None or role != "assistant":
                continue
            cleaned = clean_native_reply(content, req_id)
            if cleaned:
                parts = current.setdefault("parts", [])
                if isinstance(parts, list):
                    parts.append(cleaned)
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=False,
                    completed_at=None,
                    line_count=index,
                )
            continue

        if current is None:
            continue

        if event_type == "ContentPart":
            text = payload.get("text")
            if isinstance(text, str) and text:
                _append_part(current, text)
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=False,
                    completed_at=None,
                    line_count=index,
                )
            continue

        if event_type == "assistant.delta":
            text = payload.get("delta")
            if isinstance(text, str) and text:
                _append_part(current, text, continuous=True)
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=False,
                    completed_at=None,
                    line_count=index,
                )
            continue

        if event_type == "context.append_loop_event":
            nested = payload.get("event")
            if not isinstance(nested, dict):
                continue
            nested_type = str(nested.get("type") or "")
            if nested_type == "content.part":
                text = _text_from_value(nested.get("part"))
                if text:
                    _append_part(current, text)
                    latest = _observation_from_state(
                        path,
                        current,
                        req_id=req_id,
                        completed=False,
                        completed_at=None,
                        line_count=index,
                    )
            continue

        if event_type == "StatusUpdate":
            message_id = payload.get("message_id")
            if isinstance(message_id, str) and message_id:
                current["message_id"] = message_id
            continue

        if event_type == "TurnEnd":
            latest = _observation_from_state(
                path,
                current,
                req_id=req_id,
                completed=True,
                completed_at=timestamp,
                line_count=index,
            )
            current = None
            continue

        if event_type == "turn.ended":
            reason = str(payload.get("reason") or "").strip().lower()
            if not reason or reason == "completed":
                latest = _observation_from_state(
                    path,
                    current,
                    req_id=req_id,
                    completed=True,
                    completed_at=timestamp,
                    line_count=index,
                )
                current = None

    return latest


def _normalize_event(event: dict[str, object]) -> tuple[str, dict[str, object], object | None]:
    message = event.get("message")
    if isinstance(message, dict):
        event_type = str(message.get("type") or "")
        payload = message.get("payload")
        if not isinstance(payload, dict):
            payload = {}
        return event_type, payload, event.get("timestamp")
    event_type = str(event.get("type") or "")
    return event_type, event, event.get("timestamp") or event.get("time")


def _payload_has_req_id(payload: dict[str, object], req_id: str) -> bool:
    user_input = payload.get("user_input")
    if not isinstance(user_input, list):
        return False
    for part in user_input:
        if not isinstance(part, dict):
            continue
        text = part.get("text")
        if isinstance(text, str) and req_id in text:
            return True
    return False


def _append_part(state: dict[str, object], text: str, *, continuous: bool = False) -> None:
    parts = state.setdefault("parts", [])
    if not isinstance(parts, list):
        return
    if continuous and parts:
        parts[-1] = f"{parts[-1]}{text}"
        return
    parts.append(text)


def _value_has_req_id(value: object, req_id: str) -> bool:
    return req_id in _text_from_value(value)


def _text_from_value(value: object) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        return "\n".join(_text_from_value(item) for item in value)
    if isinstance(value, dict):
        for key in ("text", "content", "input", "user_input", "message"):
            if key in value:
                text = _text_from_value(value.get(key))
                if text:
                    return text
        return ""
    return ""


def _coerce_str(value: object) -> str | None:
    text = str(value or "").strip()
    return text or None


def _observation_from_state(
    path: Path,
    state: dict[str, object],
    *,
    req_id: str,
    completed: bool,
    completed_at: object | None,
    line_count: int,
) -> KimiTurnObservation:
    parts = state.get("parts")
    reply = clean_native_reply("\n".join(str(part) for part in parts), req_id) if isinstance(parts, list) else ""
    session_id = path.parent.name if path.parent.name else None
    message_id = state.get("message_id")
    provider_turn_ref = str(message_id).strip() if message_id else session_id
    return KimiTurnObservation(
        request_seen=True,
        completed=completed,
        reply=reply,
        session_id=session_id,
        session_path=str(path),
        provider_turn_ref=provider_turn_ref,
        line_count=line_count,
        native_started_at=state.get("started_at"),
        native_completed_at=completed_at,
    )


def _observation_sort_key(observation: KimiTurnObservation) -> tuple[float, int]:
    mtime = _path_mtime(Path(observation.session_path or ""))
    return (mtime, int(observation.line_count or 0))


def _path_mtime(path: Path) -> float:
    try:
        return path.stat().st_mtime
    except OSError:
        return -1.0


__all__ = [
    "KimiTurnObservation",
    "kimi_project_hash",
    "kimi_sessions_root",
    "observe_kimi_turn",
]
