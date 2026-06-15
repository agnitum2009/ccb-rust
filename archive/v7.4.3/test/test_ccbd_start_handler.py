from __future__ import annotations

import importlib.util
from pathlib import Path
import threading
from types import SimpleNamespace


def _build_start_handler():
    path = Path(__file__).resolve().parents[1] / 'lib' / 'ccbd' / 'handlers' / 'start.py'
    spec = importlib.util.spec_from_file_location('ccbd_start_handler_for_test', path)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module.build_start_handler


def test_start_handler_defaults_missing_auto_permission_to_plain_ccb_policy() -> None:
    start_calls: list[dict[str, object]] = []
    policies: list[dict[str, object]] = []

    app = SimpleNamespace(
        start_maintenance_lock=threading.Lock(),
        runtime_supervisor=SimpleNamespace(
            start=lambda **kwargs: start_calls.append(kwargs)
            or SimpleNamespace(to_record=lambda: {'started': []})
        ),
        persist_start_policy=lambda **kwargs: policies.append(kwargs),
    )

    payload = _build_start_handler()(app)({'agent_names': []})

    assert payload == {'started': []}
    assert start_calls[0]['restore'] is True
    assert start_calls[0]['auto_permission'] is True
    assert policies == [{'auto_permission': True, 'source': 'start_command'}]


def test_start_handler_preserves_explicit_safe_auto_permission_false() -> None:
    start_calls: list[dict[str, object]] = []
    policies: list[dict[str, object]] = []

    app = SimpleNamespace(
        start_maintenance_lock=threading.Lock(),
        runtime_supervisor=SimpleNamespace(
            start=lambda **kwargs: start_calls.append(kwargs)
            or SimpleNamespace(to_record=lambda: {'started': []})
        ),
        persist_start_policy=lambda **kwargs: policies.append(kwargs),
    )

    _build_start_handler()(app)({'agent_names': [], 'restore': False, 'auto_permission': False})

    assert start_calls[0]['restore'] is False
    assert start_calls[0]['auto_permission'] is False
    assert policies == [{'auto_permission': False, 'source': 'start_command'}]
