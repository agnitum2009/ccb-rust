from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ParsedStartCommand:
    project: str | None
    agent_names: tuple[str, ...]
    restore: bool
    auto_permission: bool
    reset_context: bool = False
    kind: str = 'start'


@dataclass(frozen=True)
class ParsedKillCommand:
    project: str | None
    force: bool = False
    kind: str = 'kill'


@dataclass(frozen=True)
class ParsedClearCommand:
    project: str | None
    agent_names: tuple[str, ...] = ()
    kind: str = 'clear'


@dataclass(frozen=True)
class ParsedRestartCommand:
    project: str | None
    agent_name: str
    kind: str = 'restart'


@dataclass(frozen=True)
class ParsedMaintenanceCommand:
    project: str | None
    action: str = 'status'
    args: tuple[str, ...] = ()
    kind: str = 'maintenance'


@dataclass(frozen=True)
class ParsedCleanupCommand:
    project: str | None
    kind: str = 'cleanup'


@dataclass(frozen=True)
class ParsedPsCommand:
    project: str | None
    alive_only: bool = False
    kind: str = 'ps'


@dataclass(frozen=True)
class ParsedConfigValidateCommand:
    project: str | None
    kind: str = 'config-validate'


@dataclass(frozen=True)
class ParsedReloadCommand:
    project: str | None
    dry_run: bool = False
    kind: str = 'reload'


@dataclass(frozen=True)
class ParsedDoctorCommand:
    project: str | None
    bundle: bool = False
    output_path: str | None = None
    storage: bool = False
    json_output: bool = False
    kind: str = 'doctor'


@dataclass(frozen=True)
class ParsedLogsCommand:
    project: str | None
    agent_name: str
    kind: str = 'logs'


@dataclass(frozen=True)
class ParsedPingCommand:
    project: str | None
    target: str
    kind: str = 'ping'


__all__ = [
    'ParsedClearCommand',
    'ParsedCleanupCommand',
    'ParsedConfigValidateCommand',
    'ParsedDoctorCommand',
    'ParsedKillCommand',
    'ParsedLogsCommand',
    'ParsedMaintenanceCommand',
    'ParsedPingCommand',
    'ParsedPsCommand',
    'ParsedReloadCommand',
    'ParsedRestartCommand',
    'ParsedStartCommand',
]
