from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class AdditiveRuntimeMountResult:
    status: str
    requested_agents: tuple[str, ...] = ()
    mounted_agents: tuple[str, ...] = ()
    runtime_authority_written_agents: tuple[str, ...] = ()
    unloaded_agents: tuple[str, ...] = ()
    runtime_authority_stopped_agents: tuple[str, ...] = ()
    helper_terminated_agents: tuple[str, ...] = ()
    preserved_runtime_unchanged_agents: tuple[str, ...] = ()
    partial: bool = False
    summary: dict[str, object] | None = None
    diagnostics: dict[str, object] = field(default_factory=dict)

    def to_record(self) -> dict[str, object]:
        return {
            'status': self.status,
            'requested_agents': list(self.requested_agents),
            'mounted_agents': list(self.mounted_agents),
            'runtime_authority_written_agents': list(
                self.runtime_authority_written_agents
            ),
            'unloaded_agents': list(self.unloaded_agents),
            'runtime_authority_stopped_agents': list(
                self.runtime_authority_stopped_agents
            ),
            'helper_terminated_agents': list(self.helper_terminated_agents),
            'preserved_runtime_unchanged_agents': list(
                self.preserved_runtime_unchanged_agents
            ),
            'partial': bool(self.partial),
            'summary': dict(self.summary) if self.summary is not None else None,
            'diagnostics': dict(self.diagnostics),
        }


def blocked_mount_result(
    reason: str,
    message: str,
    *,
    requested_agents: tuple[str, ...] = (),
) -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='blocked',
        requested_agents=requested_agents,
        diagnostics={
            'reason': reason,
            'message': message,
            **_no_publish_diagnostics(),
        },
    )


def noop_mount_result(preserved_agents: tuple[str, ...]) -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='noop',
        preserved_runtime_unchanged_agents=preserved_agents,
        diagnostics={
            'reason': 'no_new_agent_panes',
            **_no_publish_diagnostics(),
        },
    )


def failed_mount_result(
    *,
    reason: str,
    error: Exception,
    requested_agents: tuple[str, ...],
    mounted_agents: tuple[str, ...],
    written_agents: tuple[str, ...],
    preserved_unchanged_agents: tuple[str, ...],
    preserved_changed_agents: tuple[str, ...],
    summary: dict[str, object] | None,
) -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='failed',
        requested_agents=requested_agents,
        mounted_agents=mounted_agents,
        runtime_authority_written_agents=written_agents,
        preserved_runtime_unchanged_agents=preserved_unchanged_agents,
        partial=bool(written_agents),
        summary=summary,
        diagnostics={
            'reason': reason,
            'error_type': type(error).__name__,
            'error': str(error),
            'runtime_authority_scope': (
                'new_agents_only'
                if not preserved_changed_agents
                else 'preserved_agent_changed'
            ),
            'preserved_runtime_changed_agents': list(preserved_changed_agents),
            **_no_publish_diagnostics(),
        },
    )


def mounted_result(
    *,
    requested_agents: tuple[str, ...],
    mounted_agents: tuple[str, ...],
    written_agents: tuple[str, ...],
    preserved_agents: tuple[str, ...],
    summary: dict[str, object] | None,
) -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='mounted',
        requested_agents=requested_agents,
        mounted_agents=mounted_agents,
        runtime_authority_written_agents=written_agents,
        preserved_runtime_unchanged_agents=preserved_agents,
        partial=False,
        summary=summary,
        diagnostics={
            'reason': None,
            'runtime_authority_scope': 'new_agents_only',
            **_no_publish_diagnostics(),
        },
    )


def unloaded_result(
    *,
    requested_agents: tuple[str, ...],
    unloaded_agents: tuple[str, ...],
    stopped_agents: tuple[str, ...],
    helper_terminated_agents: tuple[str, ...],
    preserved_agents: tuple[str, ...],
) -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='unloaded',
        requested_agents=requested_agents,
        unloaded_agents=unloaded_agents,
        runtime_authority_stopped_agents=stopped_agents,
        helper_terminated_agents=helper_terminated_agents,
        preserved_runtime_unchanged_agents=preserved_agents,
        partial=False,
        diagnostics={
            'reason': None,
            'runtime_authority_scope': 'removed_agents_only',
            'unload_or_replace_executed': bool(unloaded_agents or stopped_agents),
            **_no_publish_diagnostics(),
        },
    )


def _no_publish_diagnostics() -> dict[str, object]:
    return {
        'graph_published': False,
        'lease_or_lifecycle_written': False,
        'cleanup_tmux_orphans': False,
        'config_watch_started': False,
    }


__all__ = [
    'AdditiveRuntimeMountResult',
    'blocked_mount_result',
    'failed_mount_result',
    'mounted_result',
    'noop_mount_result',
    'unloaded_result',
]
