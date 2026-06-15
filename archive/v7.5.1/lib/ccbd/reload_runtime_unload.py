from __future__ import annotations

from agents.models import AgentState
from ccbd.reload_runtime_mount_models import AdditiveRuntimeMountResult, blocked_mount_result, unloaded_result
from provider_runtime.helper_cleanup import terminate_helper_manifest_path


def run_removed_agent_unloads(
    app,
    graph,
    *,
    patch_result,
) -> AdditiveRuntimeMountResult:
    removed_agents = tuple(sorted((getattr(patch_result, 'removed_agents', {}) or {}).keys()))
    preserved_agents = tuple(sorted((getattr(patch_result, 'preserved_before', {}) or {}).keys()))
    if not removed_agents:
        return AdditiveRuntimeMountResult(
            status='noop',
            preserved_runtime_unchanged_agents=preserved_agents,
            diagnostics={
                'reason': 'no_removed_agent_panes',
                'graph_published': False,
                'lease_or_lifecycle_written': False,
                'config_watch_started': False,
                'cleanup_tmux_orphans': False,
                'unload_or_replace_executed': False,
            },
        )
    blocked = _unload_blocker(app, graph, removed_agents)
    if blocked is not None:
        return blocked_mount_result(*blocked, requested_agents=removed_agents)
    stopped: list[str] = []
    helpers: list[str] = []
    registry = graph.registry
    for agent_name in removed_agents:
        if terminate_helper_manifest_path(app.paths.agent_helper_path(agent_name)):
            helpers.append(agent_name)
        if registry.remove(agent_name) is not None:
            stopped.append(agent_name)
    return unloaded_result(
        requested_agents=removed_agents,
        unloaded_agents=removed_agents,
        stopped_agents=tuple(stopped),
        helper_terminated_agents=tuple(helpers),
        preserved_agents=preserved_agents,
    )


def pre_namespace_unload_blocker(app, graph, plan: dict[str, object]) -> tuple[str, str] | None:
    agents = tuple(
        sorted(
            {
                str(item.get('agent') or '').strip()
                for item in tuple(plan.get('operations') or ())
                if isinstance(item, dict) and str(item.get('op') or '') == 'remove_agent'
            }
        )
    )
    if not agents:
        return None
    return _unload_blocker(app, graph, agents)


def _unload_blocker(app, graph, agents: tuple[str, ...]) -> tuple[str, str] | None:
    dispatcher = getattr(app, 'dispatcher', None)
    has_outstanding = getattr(dispatcher, '_has_outstanding_work', None)
    for agent_name in agents:
        if callable(has_outstanding) and has_outstanding(agent_name):
            return (
                'agent_has_outstanding_work',
                f'cannot unload agent with outstanding work: {agent_name}',
            )
        runtime = graph.registry.get(agent_name)
        if runtime is not None and runtime.state is AgentState.BUSY:
            return ('agent_busy', f'cannot unload busy agent: {agent_name}')
    return None


__all__ = ['pre_namespace_unload_blocker', 'run_removed_agent_unloads']
