from __future__ import annotations

from ccbd.start_runtime.layout import cmd_bootstrap_command


def test_cmd_bootstrap_command_uses_user_zsh_directly(monkeypatch) -> None:
    monkeypatch.setenv('SHELL', 'zsh')
    monkeypatch.delenv('CCB_CMD_SHELL', raising=False)
    monkeypatch.setattr('ccbd.start_runtime.layout.shutil.which', lambda name: '/mock/bin/zsh' if name == 'zsh' else None)

    assert cmd_bootstrap_command() == 'exec /mock/bin/zsh -l'


def test_cmd_bootstrap_command_is_shell_language_agnostic_for_fish(monkeypatch) -> None:
    monkeypatch.setenv('SHELL', 'fish')
    monkeypatch.delenv('CCB_CMD_SHELL', raising=False)
    monkeypatch.setattr('ccbd.start_runtime.layout.shutil.which', lambda name: '/mock/bin/fish' if name == 'fish' else None)

    assert cmd_bootstrap_command() == 'exec /mock/bin/fish -l'
