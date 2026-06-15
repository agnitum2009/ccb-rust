from __future__ import annotations

from ccbd.reload_apply_results import not_published_diagnostics
from ccbd.reload_runtime_mount import (
    AdditiveRuntimeMountResult,
    run_additive_agent_mounts,
)
from ccbd.reload_runtime_unload import run_removed_agent_unloads


PUBLISH_READY_RUNTIME_STATUSES = frozenset({'mounted', 'noop', 'unloaded'})


def run_runtime_mount(
    app,
    target_graph,
    *,
    old_graph=None,
    namespace,
    namespace_patch,
    run_runtime_mount_fn,
    run_start_flow_fn,
):
    if run_runtime_mount_fn is not None:
        try:
            return run_runtime_mount_fn(
                app,
                target_graph,
                namespace=namespace,
                patch_result=namespace_patch,
            )
        except Exception as exc:
            return exception_runtime_mount_result(exc)
    kwargs = {
        'namespace': namespace,
        'patch_result': namespace_patch,
    }
    if run_start_flow_fn is not None:
        kwargs['run_start_flow_fn'] = run_start_flow_fn
    try:
        if getattr(namespace_patch, 'removed_agents', None):
            return run_removed_agent_unloads(
                app,
                old_graph or app.current_service_graph(),
                patch_result=namespace_patch,
            )
        return run_additive_agent_mounts(app, target_graph, **kwargs)
    except Exception as exc:
        return exception_runtime_mount_result(exc)


def exception_runtime_mount_result(exc: Exception) -> AdditiveRuntimeMountResult:
    return AdditiveRuntimeMountResult(
        status='failed',
        diagnostics={
            'reason': 'runtime_mount_failed',
            'error_type': type(exc).__name__,
            'error': str(exc),
            **not_published_diagnostics(),
        },
    )


__all__ = [
    'PUBLISH_READY_RUNTIME_STATUSES',
    'exception_runtime_mount_result',
    'run_runtime_mount',
]
