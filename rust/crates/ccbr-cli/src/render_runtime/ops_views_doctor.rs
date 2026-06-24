//! Mirrors Python `lib/cli/render_runtime/ops_views_doctor.py`.

use serde_json::Value;

use super::ops_views_common::binding_line;

/// Render a doctor payload.
///
/// Mirrors Python `render_doctor(payload)`.
pub fn render_doctor(payload: &Value) -> Vec<String> {
    let installation = payload.get("installation").unwrap_or(&Value::Null);
    let runtime = payload.get("runtime").unwrap_or(&Value::Null);
    let requirements = payload.get("requirements").unwrap_or(&Value::Null);
    let ccbrd = payload.get("ccbrd").unwrap_or(&Value::Null);

    let mut lines = vec![
        format!("project: {}", field(payload, "project")),
        format!("project_id: {}", field(payload, "project_id")),
        format!("install_path: {}", field(installation, "path")),
        format!("install_mode: {}", field(installation, "install_mode")),
        format!(
            "install_source_kind: {}",
            field(installation, "source_kind")
        ),
        format!("install_version: {}", field(installation, "version")),
        format!("install_channel: {}", field(installation, "channel")),
        format!("install_build_time: {}", field(installation, "build_time")),
        format!("install_platform: {}", field(installation, "platform")),
        format!("install_arch: {}", field(installation, "arch")),
        format!("user_id: {}", field(runtime, "user_id")),
        format!("user_name: {}", field(runtime, "user_name")),
        format!("home: {}", field(runtime, "home")),
        format!("root_runtime: {}", field(runtime, "root_runtime")),
        format!(
            "install_root_owned: {}",
            bool_field(runtime, "install_root_owned")
        ),
        format!("install_user_id: {}", field(runtime, "install_user_id")),
        format!("install_user_name: {}", field(runtime, "install_user_name")),
        format!("sudo_user: {}", field(runtime, "sudo_user")),
        format!("project_owner: {}", field(runtime, "project_owner")),
        format!("ccbr_dir_owner: {}", field(runtime, "ccbr_dir_owner")),
        format!("install_owner: {}", field(runtime, "install_owner")),
        format!(
            "requirement_python_executable: {}",
            field(requirements, "python_executable")
        ),
        format!(
            "requirement_python_version: {}",
            field(requirements, "python_version")
        ),
        format!(
            "requirement_tmux_available: {}",
            bool_field(requirements, "tmux_available")
        ),
        format!(
            "requirement_tmux_path: {}",
            field(requirements, "tmux_path")
        ),
        format!("ccbrd_state: {}", field(ccbrd, "state")),
        format!("ccbrd_socket_path: {}", field(ccbrd, "socket_path")),
        format!(
            "ccbrd_project_anchor_path: {}",
            field(ccbrd, "project_anchor_path")
        ),
        format!(
            "ccbrd_runtime_state_root: {}",
            field(ccbrd, "runtime_state_root")
        ),
        format!(
            "ccbrd_runtime_root_kind: {}",
            field(ccbrd, "runtime_root_kind")
        ),
        format!(
            "ccbrd_runtime_relocation_reason: {}",
            field(ccbrd, "runtime_relocation_reason")
        ),
        format!(
            "ccbrd_runtime_filesystem_hint: {}",
            field(ccbrd, "runtime_filesystem_hint")
        ),
        format!(
            "ccbrd_runtime_marker_status: {}",
            field(ccbrd, "runtime_marker_status")
        ),
        format!(
            "ccbrd_preferred_socket_path: {}",
            field(ccbrd, "preferred_socket_path")
        ),
        format!(
            "ccbrd_effective_socket_path: {}",
            field(ccbrd, "effective_socket_path")
        ),
        format!(
            "ccbrd_preferred_socket_path_bytes: {}",
            field(ccbrd, "preferred_socket_path_bytes")
        ),
        format!(
            "ccbrd_effective_socket_path_bytes: {}",
            field(ccbrd, "effective_socket_path_bytes")
        ),
        format!(
            "ccbrd_socket_root_kind: {}",
            field(ccbrd, "socket_root_kind")
        ),
        format!(
            "ccbrd_socket_fallback_reason: {}",
            field(ccbrd, "socket_fallback_reason")
        ),
        format!(
            "ccbrd_socket_filesystem_hint: {}",
            field(ccbrd, "socket_filesystem_hint")
        ),
        format!(
            "ccbrd_tmux_socket_path: {}",
            field(ccbrd, "tmux_socket_path")
        ),
        format!(
            "ccbrd_tmux_preferred_socket_path: {}",
            field(ccbrd, "tmux_preferred_socket_path")
        ),
        format!(
            "ccbrd_tmux_effective_socket_path: {}",
            field(ccbrd, "tmux_effective_socket_path")
        ),
        format!(
            "ccbrd_tmux_preferred_socket_path_bytes: {}",
            field(ccbrd, "tmux_preferred_socket_path_bytes")
        ),
        format!(
            "ccbrd_tmux_effective_socket_path_bytes: {}",
            field(ccbrd, "tmux_effective_socket_path_bytes")
        ),
        format!(
            "ccbrd_tmux_start_server_command: {}",
            field(ccbrd, "tmux_start_server_command")
        ),
        format!(
            "ccbrd_tmux_socket_root_kind: {}",
            field(ccbrd, "tmux_socket_root_kind")
        ),
        format!(
            "ccbrd_tmux_socket_fallback_reason: {}",
            field(ccbrd, "tmux_socket_fallback_reason")
        ),
        format!(
            "ccbrd_tmux_socket_filesystem_hint: {}",
            field(ccbrd, "tmux_socket_filesystem_hint")
        ),
        format!("ccbrd_health: {}", field(ccbrd, "health")),
        format!("ccbrd_generation: {}", field(ccbrd, "generation")),
        format!(
            "ccbrd_last_heartbeat_at: {}",
            field(ccbrd, "last_heartbeat_at")
        ),
        format!("ccbrd_pid_alive: {}", bool_field(ccbrd, "pid_alive")),
        format!(
            "ccbrd_socket_connectable: {}",
            bool_field(ccbrd, "socket_connectable")
        ),
        format!(
            "ccbrd_heartbeat_fresh: {}",
            bool_field(ccbrd, "heartbeat_fresh")
        ),
        format!(
            "ccbrd_takeover_allowed: {}",
            bool_field(ccbrd, "takeover_allowed")
        ),
        format!("ccbrd_reason: {}", field(ccbrd, "reason")),
        format!(
            "ccbrd_last_request_queue_wait_s: {}",
            field(ccbrd, "last_request_queue_wait_s")
        ),
        format!(
            "ccbrd_last_submit_duration_s: {}",
            field(ccbrd, "last_submit_duration_s")
        ),
        format!(
            "ccbrd_last_ping_duration_s: {}",
            field(ccbrd, "last_ping_duration_s")
        ),
        format!(
            "ccbrd_last_handler_latency_s_by_op: {}",
            format_mapping(ccbrd.get("last_handler_latency_s_by_op"))
        ),
        format!(
            "ccbrd_last_maintenance_duration_s: {}",
            field(ccbrd, "last_maintenance_duration_s")
        ),
        format!(
            "ccbrd_last_heartbeat_duration_s: {}",
            field(ccbrd, "last_heartbeat_duration_s")
        ),
        format!(
            "ccbrd_heartbeat_step_duration_s: {}",
            format_mapping(ccbrd.get("heartbeat_step_duration_s"))
        ),
        format!(
            "ccbrd_last_heartbeat_agents_inspected: {}",
            field(ccbrd, "last_heartbeat_agents_inspected")
        ),
        format!(
            "ccbrd_last_heartbeat_runtime_store_writes: {}",
            field(ccbrd, "last_heartbeat_runtime_store_writes")
        ),
        format!(
            "ccbrd_pending_maintenance_ticks: {}",
            field(ccbrd, "pending_maintenance_ticks")
        ),
        format!(
            "ccbrd_last_project_view_response_duration_s: {}",
            field(ccbrd, "last_project_view_response_duration_s")
        ),
        format!(
            "ccbrd_last_project_view_build_duration_s: {}",
            field(ccbrd, "last_project_view_build_duration_s")
        ),
        format!(
            "ccbrd_project_view_cache_hits: {}",
            field(ccbrd, "project_view_cache_hits")
        ),
        format!(
            "ccbrd_project_view_cache_misses: {}",
            field(ccbrd, "project_view_cache_misses")
        ),
        format!(
            "ccbrd_last_project_view_tmux_command_count: {}",
            field(ccbrd, "last_project_view_tmux_command_count")
        ),
        format!(
            "ccbrd_last_project_view_capture_pane_count: {}",
            field(ccbrd, "last_project_view_capture_pane_count")
        ),
        format!(
            "ccbrd_last_project_view_store_scan_count: {}",
            field(ccbrd, "last_project_view_store_scan_count")
        ),
        format!("ccbrd_rss_bytes: {}", field(ccbrd, "rss_bytes")),
        format!(
            "ccbrd_virtual_memory_bytes: {}",
            field(ccbrd, "virtual_memory_bytes")
        ),
        format!("ccbrd_fd_count: {}", field(ccbrd, "fd_count")),
        format!("ccbrd_thread_count: {}", field(ccbrd, "thread_count")),
        format!(
            "ccbrd_service_graph_version: {}",
            field(ccbrd, "service_graph_version")
        ),
        format!(
            "ccbrd_service_graph_created_at: {}",
            field(ccbrd, "service_graph_created_at")
        ),
        format!(
            "ccbrd_service_graph_retained_count: {}",
            field(ccbrd, "service_graph_retained_count")
        ),
        format!(
            "ccbrd_service_graph_retained_count_scope: {}",
            field(ccbrd, "service_graph_retained_count_scope")
        ),
        format!(
            "ccbrd_last_reload_duration_s: {}",
            field(ccbrd, "last_reload_duration_s")
        ),
        format!(
            "ccbrd_last_reload_plan_class: {}",
            field(ccbrd, "last_reload_plan_class")
        ),
        format!(
            "ccbrd_last_reload_error: {}",
            field(ccbrd, "last_reload_error")
        ),
        format!(
            "ccbrd_active_execution_count: {}",
            field(ccbrd, "active_execution_count")
        ),
        format!(
            "ccbrd_recoverable_execution_count: {}",
            field(ccbrd, "recoverable_execution_count")
        ),
        format!(
            "ccbrd_nonrecoverable_execution_count: {}",
            field(ccbrd, "nonrecoverable_execution_count")
        ),
        format!(
            "ccbrd_pending_items_count: {}",
            field(ccbrd, "pending_items_count")
        ),
        format!(
            "ccbrd_terminal_pending_count: {}",
            field(ccbrd, "terminal_pending_count")
        ),
        format!(
            "ccbrd_recoverable_execution_providers: {}",
            field(ccbrd, "recoverable_execution_providers")
        ),
        format!(
            "ccbrd_nonrecoverable_execution_providers: {}",
            field(ccbrd, "nonrecoverable_execution_providers")
        ),
        format!("ccbrd_last_restore_at: {}", field(ccbrd, "last_restore_at")),
        format!(
            "ccbrd_last_restore_running_job_count: {}",
            field(ccbrd, "last_restore_running_job_count")
        ),
        format!(
            "ccbrd_last_restore_restored_execution_count: {}",
            field(ccbrd, "last_restore_restored_execution_count")
        ),
        format!(
            "ccbrd_last_restore_replay_pending_count: {}",
            field(ccbrd, "last_restore_replay_pending_count")
        ),
        format!(
            "ccbrd_last_restore_terminal_pending_count: {}",
            field(ccbrd, "last_restore_terminal_pending_count")
        ),
        format!(
            "ccbrd_last_restore_abandoned_execution_count: {}",
            field(ccbrd, "last_restore_abandoned_execution_count")
        ),
        format!(
            "ccbrd_last_restore_already_active_count: {}",
            field(ccbrd, "last_restore_already_active_count")
        ),
        format!(
            "ccbrd_last_restore_results_text: {}",
            field(ccbrd, "last_restore_results_text")
        ),
        format!("ccbrd_startup_last_at: {}", field(ccbrd, "startup_last_at")),
        format!(
            "ccbrd_startup_last_trigger: {}",
            field(ccbrd, "startup_last_trigger")
        ),
        format!(
            "ccbrd_startup_last_status: {}",
            field(ccbrd, "startup_last_status")
        ),
        format!(
            "ccbrd_startup_last_generation: {}",
            field(ccbrd, "startup_last_generation")
        ),
        format!(
            "ccbrd_startup_last_daemon_started: {}",
            bool_field(ccbrd, "startup_last_daemon_started")
        ),
        format!(
            "ccbrd_startup_last_requested_agents: {}",
            field(ccbrd, "startup_last_requested_agents")
        ),
        format!(
            "ccbrd_startup_last_desired_agents: {}",
            field(ccbrd, "startup_last_desired_agents")
        ),
        format!(
            "ccbrd_startup_last_actions: {}",
            field(ccbrd, "startup_last_actions")
        ),
        format!(
            "ccbrd_startup_last_cleanup_killed: {}",
            field(ccbrd, "startup_last_cleanup_killed")
        ),
        format!(
            "ccbrd_startup_last_failure_reason: {}",
            field(ccbrd, "startup_last_failure_reason")
        ),
        format!(
            "ccbrd_startup_last_agent_results_text: {}",
            field(ccbrd, "startup_last_agent_results_text")
        ),
        format!(
            "ccbrd_shutdown_last_at: {}",
            field(ccbrd, "shutdown_last_at")
        ),
        format!(
            "ccbrd_shutdown_last_trigger: {}",
            field(ccbrd, "shutdown_last_trigger")
        ),
        format!(
            "ccbrd_shutdown_last_status: {}",
            field(ccbrd, "shutdown_last_status")
        ),
        format!(
            "ccbrd_shutdown_last_forced: {}",
            bool_field(ccbrd, "shutdown_last_forced")
        ),
        format!(
            "ccbrd_shutdown_last_generation: {}",
            field(ccbrd, "shutdown_last_generation")
        ),
        format!(
            "ccbrd_shutdown_last_reason: {}",
            field(ccbrd, "shutdown_last_reason")
        ),
        format!(
            "ccbrd_shutdown_last_stopped_agents: {}",
            field(ccbrd, "shutdown_last_stopped_agents")
        ),
        format!(
            "ccbrd_shutdown_last_actions: {}",
            field(ccbrd, "shutdown_last_actions")
        ),
        format!(
            "ccbrd_shutdown_last_cleanup_killed: {}",
            field(ccbrd, "shutdown_last_cleanup_killed")
        ),
        format!(
            "ccbrd_shutdown_last_failure_reason: {}",
            field(ccbrd, "shutdown_last_failure_reason")
        ),
        format!(
            "ccbrd_shutdown_last_runtime_states_text: {}",
            field(ccbrd, "shutdown_last_runtime_states_text")
        ),
        format!("ccbrd_namespace_epoch: {}", field(ccbrd, "namespace_epoch")),
        format!(
            "ccbrd_namespace_tmux_socket_path: {}",
            field(ccbrd, "namespace_tmux_socket_path")
        ),
        format!(
            "ccbrd_namespace_tmux_session_name: {}",
            field(ccbrd, "namespace_tmux_session_name")
        ),
        format!(
            "ccbrd_namespace_layout_version: {}",
            field(ccbrd, "namespace_layout_version")
        ),
        format!(
            "ccbrd_namespace_ui_attachable: {}",
            bool_field(ccbrd, "namespace_ui_attachable")
        ),
        format!(
            "ccbrd_namespace_last_started_at: {}",
            field(ccbrd, "namespace_last_started_at")
        ),
        format!(
            "ccbrd_namespace_last_destroyed_at: {}",
            field(ccbrd, "namespace_last_destroyed_at")
        ),
        format!(
            "ccbrd_namespace_last_destroy_reason: {}",
            field(ccbrd, "namespace_last_destroy_reason")
        ),
        format!(
            "ccbrd_namespace_last_event_kind: {}",
            field(ccbrd, "namespace_last_event_kind")
        ),
        format!(
            "ccbrd_namespace_last_event_at: {}",
            field(ccbrd, "namespace_last_event_at")
        ),
        format!(
            "ccbrd_namespace_last_event_epoch: {}",
            field(ccbrd, "namespace_last_event_epoch")
        ),
        format!(
            "ccbrd_namespace_last_event_socket_path: {}",
            field(ccbrd, "namespace_last_event_socket_path")
        ),
        format!(
            "ccbrd_namespace_last_event_session_name: {}",
            field(ccbrd, "namespace_last_event_session_name")
        ),
        format!(
            "ccbrd_start_policy_auto_permission: {}",
            field(ccbrd, "start_policy_auto_permission")
        ),
        format!(
            "ccbrd_start_policy_recovery_restore: {}",
            field(ccbrd, "start_policy_recovery_restore")
        ),
        format!(
            "ccbrd_start_policy_last_started_at: {}",
            field(ccbrd, "start_policy_last_started_at")
        ),
        format!(
            "ccbrd_start_policy_source: {}",
            field(ccbrd, "start_policy_source")
        ),
        format!(
            "ccbrd_tmux_cleanup_last_kind: {}",
            field(ccbrd, "tmux_cleanup_last_kind")
        ),
        format!(
            "ccbrd_tmux_cleanup_last_at: {}",
            field(ccbrd, "tmux_cleanup_last_at")
        ),
        format!(
            "ccbrd_tmux_cleanup_socket_count: {}",
            field(ccbrd, "tmux_cleanup_socket_count")
        ),
        format!(
            "ccbrd_tmux_cleanup_total_owned: {}",
            field(ccbrd, "tmux_cleanup_total_owned")
        ),
        format!(
            "ccbrd_tmux_cleanup_total_active: {}",
            field(ccbrd, "tmux_cleanup_total_active")
        ),
        format!(
            "ccbrd_tmux_cleanup_total_orphaned: {}",
            field(ccbrd, "tmux_cleanup_total_orphaned")
        ),
        format!(
            "ccbrd_tmux_cleanup_total_killed: {}",
            field(ccbrd, "tmux_cleanup_total_killed")
        ),
        format!(
            "ccbrd_tmux_cleanup_sockets: {}",
            field(ccbrd, "tmux_cleanup_sockets")
        ),
    ];

    // Provider commands
    if let Some(Value::Array(providers)) = requirements.get("provider_commands") {
        for provider in providers {
            lines.push(format!(
                "requirement_provider: name={} executable={} available={} path={}",
                field(provider, "provider"),
                field(provider, "executable"),
                bool_field(provider, "available"),
                field(provider, "path")
            ));
        }
    }

    // Runtime warnings
    if let Some(Value::Array(warnings)) = runtime.get("warnings") {
        for warning in warnings {
            if let Some(w) = warning.as_str() {
                lines.push(format!("runtime_warning: {}", w));
            }
        }
    }

    // Diagnostic errors
    if let Some(Value::Array(errors)) = ccbrd.get("diagnostic_errors") {
        for error in errors {
            if let Some(e) = error.as_str() {
                lines.push(format!("ccbrd_diagnostic_error: {}", e));
            }
        }
    }

    // Agents
    if let Some(Value::Array(agents)) = payload.get("agents") {
        for agent in agents {
            lines.push(format!(
                "agent: name={} health={} provider={} completion={}",
                field(agent, "agent_name"),
                field(agent, "health"),
                field(agent, "provider"),
                field(agent, "completion_family")
            ));
            lines.push(binding_line(agent));
            lines.push(format!(
                "restore: supported={} mode={} reason={}",
                bool_field(agent, "execution_resume_supported"),
                field(agent, "execution_restore_mode"),
                field(agent, "execution_restore_reason")
            ));
            lines.push(format!(
                "restore_detail: {}",
                field(agent, "execution_restore_detail")
            ));
            lines.push(format!(
                "mailbox_summary: version={} source={} refreshed_at={} state={} queue={} pending_reply={} active={} head={} head_type={} head_status={}",
                field(agent, "mailbox_summary_version"),
                field(agent, "mailbox_summary_source"),
                field(agent, "mailbox_summary_refreshed_at"),
                field(agent, "mailbox_state"),
                field(agent, "mailbox_queue_depth"),
                field(agent, "mailbox_pending_reply_count"),
                field(agent, "mailbox_active_inbound_event_id"),
                field(agent, "mailbox_head_inbound_event_id"),
                field(agent, "mailbox_head_event_type"),
                field(agent, "mailbox_head_status")
            ));

            let projected = agent
                .get("mailbox_consistency_projected")
                .unwrap_or(&Value::Null);
            let mismatches = agent
                .get("mailbox_consistency_mismatches")
                .and_then(|v| v.as_array());
            let mismatch_items: Vec<String> = mismatches
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            lines.push(format!(
                "mailbox_consistency: status={} mismatches={} projected_state={} projected_queue={} projected_pending_reply={} projected_active={} projected_head={} projected_head_type={} projected_head_status={}",
                field(agent, "mailbox_consistency_status"),
                if mismatch_items.is_empty() { "none".to_string() } else { mismatch_items.join(",") },
                field(projected, "mailbox_state"),
                field(projected, "queue_depth"),
                field(projected, "pending_reply_count"),
                field(projected, "active_inbound_event_id"),
                field(projected, "head_inbound_event_id"),
                field(projected, "head_event_type"),
                field(projected, "head_status")
            ));

            if let Some(error) = agent
                .get("mailbox_consistency_error")
                .and_then(|v| v.as_str())
            {
                lines.push(format!("mailbox_consistency_error: {}", error));
            }

            if agent.get("session_switch_state").is_some() {
                lines.push(format!(
                    "session_switch: state={} reason={} committed={} candidate_session={} candidate_path={}",
                    field(agent, "session_switch_state"),
                    field(agent, "session_switch_reason"),
                    bool_field(agent, "session_switch_committed"),
                    field(agent, "session_switch_candidate_id"),
                    field(agent, "session_switch_candidate_path")
                ));
            }
        }
    }

    lines
}

/// Format a mapping value as "key=value,key=value" string.
///
/// Mirrors Python `_format_mapping(value)`.
fn format_mapping(value: Option<&Value>) -> String {
    let obj = match value {
        Some(Value::Object(o)) => o,
        _ => return String::new(),
    };

    let mut keys: Vec<&String> = obj.keys().collect();
    keys.sort();

    let parts: Vec<String> = keys
        .iter()
        .map(|key| format!("{}={}", key, field(value.unwrap(), key)))
        .collect();

    parts.join(",")
}

/// Render a doctor storage payload.
///
/// Mirrors Python `render_doctor_storage(payload)`.
pub fn render_doctor_storage(payload: &Value) -> Vec<String> {
    let mut lines = vec![
        "storage_status: ok".to_string(),
        format!(
            "storage_schema_version: {}",
            field(payload, "schema_version")
        ),
        format!("project: {}", field(payload, "project")),
        format!("project_id: {}", field(payload, "project_id")),
        format!(
            "storage_runtime_root_kind: {}",
            field(payload, "runtime_root_kind")
        ),
        format!(
            "storage_runtime_state_root: {}",
            field(payload, "runtime_state_root")
        ),
        format!(
            "storage_shared_cache_root: {}",
            field_or(payload, "shared_cache_root", "")
        ),
        format!(
            "storage_shared_cache_root_usable: {}",
            bool_field(payload, "shared_cache_root_usable")
        ),
        format!(
            "storage_shared_cache_status: {}",
            field(payload, "shared_cache_status")
        ),
        format!(
            "storage_shared_cache_reason: {}",
            field(payload, "shared_cache_reason")
        ),
        format!("storage_total_bytes: {}", field(payload, "total_bytes")),
        format!("storage_total_count: {}", field(payload, "total_count")),
    ];

    // Storage by class
    if let Some(Value::Object(by_class)) = payload.get("by_class") {
        let mut keys: Vec<&String> = by_class.keys().collect();
        keys.sort();
        for storage_class in keys {
            if let Some(summary) = by_class.get(storage_class) {
                lines.push(format!(
                    "storage_class: class={} bytes={} count={}",
                    storage_class,
                    field(summary, "bytes"),
                    field(summary, "count")
                ));
            }
        }
    }

    // Storage by provider
    if let Some(Value::Object(by_provider)) = payload.get("by_provider") {
        let mut keys: Vec<&String> = by_provider.keys().collect();
        keys.sort();
        for provider in keys {
            if let Some(summary) = by_provider.get(provider) {
                lines.push(format!(
                    "storage_provider: provider={} bytes={} count={}",
                    provider,
                    field(summary, "bytes"),
                    field(summary, "count")
                ));
            }
        }
    }

    // Storage by agent
    if let Some(Value::Object(by_agent)) = payload.get("by_agent") {
        let mut keys: Vec<&String> = by_agent.keys().collect();
        keys.sort();
        for agent in keys {
            if let Some(summary) = by_agent.get(agent) {
                lines.push(format!(
                    "storage_agent: agent={} bytes={} count={}",
                    agent,
                    field(summary, "bytes"),
                    field(summary, "count")
                ));
            }
        }
    }

    // Storage entries (limited to 50)
    if let Some(Value::Array(entries)) = payload.get("entries") {
        for entry in entries.iter().take(50) {
            lines.push(format!(
                "storage_entry: class={} provider={} agent={} bytes={} active={} reclaimable={} reason={} path={}",
                field(entry, "storage_class"),
                field(entry, "provider"),
                field(entry, "agent"),
                field(entry, "size_bytes"),
                bool_field(entry, "active"),
                bool_field(entry, "reclaimable"),
                field(entry, "reason"),
                field(entry, "relative_path")
            ));
        }
    }

    lines
}

/// Get a string field from a value.
fn field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Null) => String::new(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// Get a string field or a default if empty.
fn field_or(value: &Value, key: &str, default: &str) -> String {
    let v = field(value, key);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
}

/// Get a boolean field from a value.
fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}
