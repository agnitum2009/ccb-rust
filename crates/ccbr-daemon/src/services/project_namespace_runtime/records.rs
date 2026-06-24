//! Mirrors Python `lib/ccbrd/services/project_namespace_runtime/records.py`.
//! 1:1 file alignment stub.

use super::models::{
    ProjectNamespace, ProjectNamespaceDestroySummary, ProjectNamespaceEvent, ProjectNamespaceState,
};

/// Normalize layout signature to canonical form
pub fn normalized_layout_signature(signature: Option<&str>) -> Option<String> {
    signature
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[allow(clippy::too_many_arguments)]
pub fn build_active_state(
    project_id: String,
    current: Option<&ProjectNamespaceState>,
    namespace_epoch: i64,
    tmux_socket_path: String,
    tmux_session_name: String,
    layout_version: i64,
    layout_signature: Option<String>,
    control_window_name: Option<String>,
    control_window_id: Option<String>,
    workspace_window_name: Option<String>,
    workspace_window_id: Option<String>,
    workspace_epoch: i64,
    ui_attachable: bool,
    last_started_at: Option<String>,
) -> ProjectNamespaceState {
    ProjectNamespaceState {
        project_id,
        namespace_epoch,
        tmux_socket_path,
        tmux_session_name,
        layout_version,
        layout_signature,
        control_window_name,
        control_window_id,
        workspace_window_name,
        workspace_window_id,
        workspace_epoch,
        ui_attachable,
        last_started_at,
        last_destroyed_at: current.and_then(|s| s.last_destroyed_at.clone()),
        last_destroy_reason: current.and_then(|s| s.last_destroy_reason.clone()),
    }
}

pub fn build_created_event(
    project_id: String,
    occurred_at: String,
    namespace_epoch: i64,
    tmux_socket_path: String,
    tmux_session_name: String,
    recreated: bool,
    reason: String,
) -> ProjectNamespaceEvent {
    let mut details = serde_json::Map::new();
    details.insert("recreated".to_string(), serde_json::Value::Bool(recreated));
    details.insert("reason".to_string(), serde_json::Value::String(reason));
    ProjectNamespaceEvent {
        event_kind: "namespace_created".to_string(),
        project_id,
        occurred_at,
        namespace_epoch: Some(namespace_epoch),
        tmux_socket_path: Some(tmux_socket_path),
        tmux_session_name: Some(tmux_session_name),
        details,
    }
}

/// Arity mirrors the Python `records.build_destroyed_state` helper.
#[allow(clippy::too_many_arguments)]
pub fn build_destroyed_state(
    current: Option<&ProjectNamespaceState>,
    project_id: String,
    occurred_at: String,
    reason: String,
    tmux_socket_path: String,
    tmux_session_name: String,
    layout_version: i64,
    control_window_name: Option<String>,
    workspace_window_name: Option<String>,
) -> ProjectNamespaceState {
    if let Some(current) = current {
        return current.with_destroyed(occurred_at, reason);
    }
    ProjectNamespaceState {
        project_id,
        namespace_epoch: 1,
        tmux_socket_path,
        tmux_session_name,
        layout_version,
        layout_signature: None,
        control_window_name,
        control_window_id: None,
        workspace_window_name,
        workspace_window_id: None,
        workspace_epoch: 1,
        ui_attachable: false,
        last_started_at: None,
        last_destroyed_at: Some(occurred_at),
        last_destroy_reason: Some(reason),
    }
}

pub fn build_destroyed_event(
    project_id: String,
    occurred_at: String,
    namespace_epoch: i64,
    tmux_socket_path: String,
    tmux_session_name: String,
    destroyed: bool,
    reason: String,
) -> ProjectNamespaceEvent {
    let mut details = serde_json::Map::new();
    details.insert("destroyed".to_string(), serde_json::Value::Bool(destroyed));
    details.insert("reason".to_string(), serde_json::Value::String(reason));
    ProjectNamespaceEvent {
        event_kind: "namespace_destroyed".to_string(),
        project_id,
        occurred_at,
        namespace_epoch: Some(namespace_epoch),
        tmux_socket_path: Some(tmux_socket_path),
        tmux_session_name: Some(tmux_session_name),
        details,
    }
}

pub fn build_destroy_summary(
    project_id: String,
    namespace_epoch: Option<i64>,
    tmux_socket_path: String,
    tmux_session_name: String,
    destroyed: bool,
    reason: String,
) -> ProjectNamespaceDestroySummary {
    ProjectNamespaceDestroySummary {
        project_id,
        namespace_epoch,
        tmux_socket_path,
        tmux_session_name,
        destroyed,
        reason,
    }
}

pub fn namespace_from_state(state: &ProjectNamespaceState) -> ProjectNamespace {
    ProjectNamespace::from_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_layout_signature() {
        assert_eq!(normalized_layout_signature(None), None);
        assert_eq!(normalized_layout_signature(Some("  ")), None);
        assert_eq!(
            normalized_layout_signature(Some("  abc  ")),
            Some("abc".to_string())
        );
        assert_eq!(
            normalized_layout_signature(Some("abc")),
            Some("abc".to_string())
        );
        assert_eq!(normalized_layout_signature(Some("")), None);
    }

    #[test]
    fn test_build_destroyed_state_with_current() {
        let current = ProjectNamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 5,
            tmux_socket_path: "/tmp/tmux-1".to_string(),
            tmux_session_name: "sess-1".to_string(),
            layout_version: 2,
            layout_signature: Some("sig".to_string()),
            control_window_name: Some("ctrl".to_string()),
            control_window_id: Some("ctrl-id".to_string()),
            workspace_window_name: Some("ws".to_string()),
            workspace_window_id: Some("ws-id".to_string()),
            workspace_epoch: 3,
            ui_attachable: true,
            last_started_at: Some("2024-01-01".to_string()),
            last_destroyed_at: None,
            last_destroy_reason: None,
        };
        let state = build_destroyed_state(
            Some(&current),
            "p1".to_string(),
            "2024-02-01".to_string(),
            "cleanup".to_string(),
            "/tmp/tmux-1".to_string(),
            "sess-1".to_string(),
            2,
            Some("ctrl".to_string()),
            Some("ws".to_string()),
        );
        assert_eq!(state.project_id, "p1");
        assert_eq!(state.namespace_epoch, 5);
        assert!(!state.ui_attachable);
        assert_eq!(state.last_destroyed_at, Some("2024-02-01".to_string()));
        assert_eq!(state.last_destroy_reason, Some("cleanup".to_string()));
    }

    #[test]
    fn test_build_destroyed_state_without_current() {
        let state = build_destroyed_state(
            None,
            "p1".to_string(),
            "2024-02-01".to_string(),
            "cleanup".to_string(),
            "/tmp/tmux-1".to_string(),
            "sess-1".to_string(),
            2,
            Some("ctrl".to_string()),
            Some("ws".to_string()),
        );
        assert_eq!(state.namespace_epoch, 1);
        assert_eq!(state.workspace_epoch, 1);
        assert!(!state.ui_attachable);
        assert_eq!(state.last_destroyed_at, Some("2024-02-01".to_string()));
        assert_eq!(state.last_destroy_reason, Some("cleanup".to_string()));
    }

    #[test]
    fn test_build_created_event() {
        let event = build_created_event(
            "p1".to_string(),
            "2024-01-01".to_string(),
            2,
            "/tmp/tmux-1".to_string(),
            "sess-1".to_string(),
            false,
            "initial".to_string(),
        );
        assert_eq!(event.event_kind, "namespace_created");
        assert_eq!(event.project_id, "p1");
        assert_eq!(event.namespace_epoch, Some(2));
        assert_eq!(
            event.details.get("recreated").unwrap(),
            &serde_json::Value::Bool(false)
        );
        assert_eq!(
            event.details.get("reason").unwrap(),
            &serde_json::Value::String("initial".to_string())
        );
    }

    #[test]
    fn test_build_destroyed_event() {
        let event = build_destroyed_event(
            "p1".to_string(),
            "2024-01-01".to_string(),
            2,
            "/tmp/tmux-1".to_string(),
            "sess-1".to_string(),
            true,
            "shutdown".to_string(),
        );
        assert_eq!(event.event_kind, "namespace_destroyed");
        assert_eq!(
            event.details.get("destroyed").unwrap(),
            &serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn test_build_active_state_preserves_destroyed_fields() {
        let current = ProjectNamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 1,
            tmux_socket_path: "/tmp/old".to_string(),
            tmux_session_name: "old".to_string(),
            layout_version: 1,
            layout_signature: None,
            control_window_name: None,
            control_window_id: None,
            workspace_window_name: None,
            workspace_window_id: None,
            workspace_epoch: 1,
            ui_attachable: false,
            last_started_at: None,
            last_destroyed_at: Some("2024-01-01".to_string()),
            last_destroy_reason: Some("previous".to_string()),
        };
        let state = build_active_state(
            "p1".to_string(),
            Some(&current),
            2,
            "/tmp/new".to_string(),
            "new".to_string(),
            1,
            None,
            None,
            None,
            None,
            None,
            1,
            true,
            Some("2024-02-01".to_string()),
        );
        assert_eq!(state.last_destroyed_at, Some("2024-01-01".to_string()));
        assert_eq!(state.last_destroy_reason, Some("previous".to_string()));
        assert_eq!(state.tmux_socket_path, "/tmp/new");
    }

    #[test]
    fn test_namespace_from_state() {
        let state = ProjectNamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 3,
            tmux_socket_path: "/tmp/tmux-1".to_string(),
            tmux_session_name: "sess-1".to_string(),
            layout_version: 2,
            layout_signature: Some("sig".to_string()),
            control_window_name: Some("ctrl".to_string()),
            control_window_id: Some("ctrl-id".to_string()),
            workspace_window_name: Some("ws".to_string()),
            workspace_window_id: Some("ws-id".to_string()),
            workspace_epoch: 4,
            ui_attachable: true,
            last_started_at: None,
            last_destroyed_at: None,
            last_destroy_reason: None,
        };
        let ns = namespace_from_state(&state);
        assert_eq!(ns.project_id, "p1");
        assert_eq!(ns.namespace_epoch, 3);
        assert!(!ns.created_this_call);
        assert!(!ns.workspace_recreated_this_call);
    }
}
