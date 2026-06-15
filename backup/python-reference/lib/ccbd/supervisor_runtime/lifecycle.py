from __future__ import annotations

from ccbd.services import CcbdLifecycleStore
from ccbd.services.project_namespace_runtime import build_namespace_topology_plan

from .namespace import ensure_project_namespace
from .reporting import record_startup_report


def start_supervisor(
    supervisor,
    *,
    agent_names: tuple[str, ...],
    restore: bool,
    auto_permission: bool,
    terminal_size: tuple[int, int] | None,
    cleanup_tmux_orphans: bool,
    interactive_tmux_layout: bool,
    recreate_namespace: bool,
    reflow_workspace: bool,
    recreate_reason: str | None,
    background_maintenance: bool,
    run_start_flow_fn,
):
    try:
        topology_plan = None
        namespace_layout_signature = None
        if (
            supervisor._project_namespace is not None
            and _uses_explicit_windows_topology(supervisor._config, interactive_tmux_layout=interactive_tmux_layout)
        ):
            topology_plan = build_namespace_topology_plan(
                supervisor._config,
                ccbd_socket_path=str(supervisor._paths.ccbd_socket_path),
                project_root=str(supervisor._project_root),
            )
            namespace_layout_signature = topology_plan.signature
        elif supervisor._project_namespace is not None and interactive_tmux_layout:
            namespace_layout_signature = str(getattr(supervisor._config, 'topology_signature', '') or '') or None
        namespace = (
            ensure_project_namespace(
                supervisor._project_namespace,
                layout_signature=namespace_layout_signature,
                topology_plan=topology_plan,
                recreate_namespace=recreate_namespace,
                reflow_workspace=reflow_workspace,
                recreate_reason=recreate_reason,
                background_maintenance=background_maintenance,
                terminal_size=terminal_size,
            )
            if supervisor._project_namespace is not None
            else None
        )
        summary = run_start_flow_fn(
            project_root=supervisor._project_root,
            project_id=supervisor._project_id,
            paths=supervisor._paths,
            config=supervisor._config,
            runtime_service=supervisor._runtime_service,
            requested_agents=agent_names,
            restore=restore,
            auto_permission=auto_permission,
            cleanup_tmux_orphans=cleanup_tmux_orphans,
            interactive_tmux_layout=interactive_tmux_layout,
            tmux_socket_path=namespace.tmux_socket_path if namespace is not None else None,
            tmux_session_name=namespace.tmux_session_name if namespace is not None else None,
            tmux_workspace_window_name=getattr(namespace, 'workspace_window_name', None) if namespace is not None else None,
            namespace_epoch=namespace.namespace_epoch if namespace is not None else None,
            workspace_window_id=getattr(namespace, 'workspace_window_id', None) if namespace is not None else None,
            workspace_epoch=getattr(namespace, 'workspace_epoch', None) if namespace is not None else None,
            namespace_agent_panes=getattr(supervisor._project_namespace, '_last_materialized_agent_panes', None),
            namespace_active_panes=getattr(supervisor._project_namespace, '_last_topology_active_panes', None),
            fresh_namespace=bool(getattr(namespace, 'created_this_call', False)),
            fresh_workspace=bool(getattr(namespace, 'workspace_recreated_this_call', False)),
            clock=supervisor._clock,
        )
        _sync_lifecycle_namespace_epoch(supervisor, namespace=namespace)
    except Exception as exc:
        record_startup_report(
            supervisor,
            requested_agents=agent_names,
            restore=restore,
            auto_permission=auto_permission,
            status='failed',
            actions_taken=('start_flow_failed',),
            cleanup_summaries=(),
            agent_results=(),
            failure_reason=str(exc),
        )
        raise

    record_startup_report(
        supervisor,
        requested_agents=agent_names,
        restore=restore,
        auto_permission=auto_permission,
        status='ok',
        actions_taken=summary.actions_taken,
        cleanup_summaries=summary.cleanup_summaries,
        agent_results=summary.agent_results,
        failure_reason=None,
    )
    return summary


def _sync_lifecycle_namespace_epoch(supervisor, *, namespace) -> None:
    if namespace is None:
        return
    epoch = getattr(namespace, 'namespace_epoch', None)
    if epoch is None:
        return
    lifecycle_store = CcbdLifecycleStore(supervisor._paths)
    lifecycle = lifecycle_store.load()
    if lifecycle is None:
        return
    inspection = supervisor._ownership_guard.inspect()
    current_generation = inspection.generation
    if current_generation is None:
        return
    if lifecycle.generation != int(current_generation):
        return
    if lifecycle.phase == 'unmounted':
        return
    if lifecycle.namespace_epoch == int(epoch):
        return
    lifecycle_store.save(
        lifecycle.with_updates(namespace_epoch=int(epoch))
    )


def _uses_explicit_windows_topology(config, *, interactive_tmux_layout: bool) -> bool:
    return bool(interactive_tmux_layout and getattr(config, 'windows_explicit', False))


def stop_all_supervisor(
    supervisor,
    *,
    force: bool,
    cleanup_project_tmux_orphans_by_socket_fn,
    tmux_cleanup_history_store_cls,
    stop_all_project_fn,
):
    return stop_all_project_fn(
        project_root=supervisor._project_root,
        project_id=supervisor._project_id,
        paths=supervisor._paths,
        registry=supervisor._registry,
        project_namespace=supervisor._project_namespace,
        clock=supervisor._clock,
        force=force,
        cleanup_project_tmux_orphans_by_socket_fn=cleanup_project_tmux_orphans_by_socket_fn,
        tmux_cleanup_history_store_cls=tmux_cleanup_history_store_cls,
    )


__all__ = ['start_supervisor', 'stop_all_supervisor']
