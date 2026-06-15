from __future__ import annotations

from pathlib import Path

from provider_core.projected_assets import route_projected_tree

from .runtime_lookup import project_role_skill_sources


def project_role_skills_to_home(
    *,
    project_root: Path | None,
    agent_name: str | None,
    provider: str,
    target_skills_dir: Path,
) -> tuple[dict[str, object], ...]:
    if project_root is None or agent_name is None:
        return ()
    results: list[dict[str, object]] = []
    for skill_name, source, role_id in project_role_skill_sources(project_root, agent_name, provider):
        target = Path(target_skills_dir) / skill_name
        ok = route_projected_tree(
            source,
            target,
            enabled=True,
            label=f'{provider}-role-skill:{role_id}:{skill_name}',
            allow_unmarked_replace=False,
        )
        results.append(
            {
                'role_id': role_id,
                'provider': provider,
                'skill': skill_name,
                'source': str(source),
                'target': str(target),
                'status': 'ok' if ok else 'failed',
            }
        )
    return tuple(results)


__all__ = ['project_role_skills_to_home']
