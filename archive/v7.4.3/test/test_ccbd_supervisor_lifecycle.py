from __future__ import annotations

from types import SimpleNamespace

from ccbd.supervisor_runtime.lifecycle import _uses_explicit_windows_topology


def test_supervisor_uses_topology_only_for_explicit_windows_config() -> None:
    assert _uses_explicit_windows_topology(
        SimpleNamespace(windows_explicit=True),
        interactive_tmux_layout=True,
    ) is True
    assert _uses_explicit_windows_topology(
        SimpleNamespace(windows_explicit=False),
        interactive_tmux_layout=True,
    ) is False
    assert _uses_explicit_windows_topology(
        SimpleNamespace(windows_explicit=True),
        interactive_tmux_layout=False,
    ) is False
