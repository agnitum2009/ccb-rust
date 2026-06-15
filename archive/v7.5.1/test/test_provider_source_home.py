from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

import provider_core.source_home as source_home


def test_current_provider_source_home_uses_home_when_not_managed(tmp_path: Path, monkeypatch) -> None:
    home = tmp_path / 'home'
    monkeypatch.setenv('HOME', str(home))

    assert source_home.current_provider_source_home() == home


def test_current_provider_source_home_falls_back_from_managed_home_to_account_home(
    tmp_path: Path,
    monkeypatch,
) -> None:
    account_home = tmp_path / 'account-home'
    managed_home = tmp_path / 'repo' / '.ccb' / 'agents' / 'agent1' / 'provider-state' / 'claude' / 'home'
    monkeypatch.setenv('HOME', str(managed_home))
    monkeypatch.delenv('CCB_SOURCE_HOME', raising=False)
    if source_home.pwd is not None:
        monkeypatch.setattr(source_home.pwd, 'getpwuid', lambda _uid: SimpleNamespace(pw_dir=str(account_home)))
    else:
        monkeypatch.setenv('USERPROFILE', str(account_home))

    assert source_home.current_provider_source_home() == account_home


def test_current_provider_source_home_honors_explicit_override(tmp_path: Path, monkeypatch) -> None:
    override = tmp_path / 'override-home'
    managed_home = tmp_path / 'repo' / '.ccb' / 'agents' / 'agent1' / 'provider-state' / 'claude' / 'home'
    monkeypatch.setenv('HOME', str(managed_home))
    monkeypatch.setenv('CCB_SOURCE_HOME', str(override))

    assert source_home.current_provider_source_home() == override
