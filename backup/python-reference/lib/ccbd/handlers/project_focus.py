from __future__ import annotations


def build_project_focus_window_handler(project_focus_service):
    def handle(payload: dict) -> dict:
        return project_focus_service.focus_window(
            window=str(payload.get('window') or ''),
            namespace_epoch=_optional_int(payload.get('namespace_epoch')),
        )

    return handle


def build_project_focus_agent_handler(project_focus_service):
    def handle(payload: dict) -> dict:
        return project_focus_service.focus_agent(
            agent=str(payload.get('agent') or ''),
            namespace_epoch=_optional_int(payload.get('namespace_epoch')),
        )

    return handle


def _optional_int(value) -> int | None:
    if value is None:
        return None
    text = str(value).strip()
    return int(text) if text else None


__all__ = ['build_project_focus_agent_handler', 'build_project_focus_window_handler']
