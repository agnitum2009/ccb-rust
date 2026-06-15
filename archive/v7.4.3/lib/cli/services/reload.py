from __future__ import annotations

from cli.context import CliContext
from cli.models import ParsedReloadCommand

from .daemon import connect_current_mounted_daemon
from .reload_handoff import begin_cli_reload_handoff, clear_cli_reload_handoff


def reload_config(context: CliContext, command: ParsedReloadCommand) -> dict:
    handoff_started = False if bool(command.dry_run) else begin_cli_reload_handoff(context)
    try:
        handle = connect_current_mounted_daemon(context)
        assert handle.client is not None
        return handle.client.project_reload_config(dry_run=bool(command.dry_run))
    finally:
        if handoff_started:
            clear_cli_reload_handoff(context)


__all__ = ['reload_config']
