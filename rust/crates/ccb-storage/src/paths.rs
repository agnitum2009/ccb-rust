use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::atomic::atomic_write_json;
use crate::path_helpers::{
    choose_runtime_state_placement, choose_socket_placement, normalize_agent_name,
    normalized_segment, read_runtime_root_marker_payload, read_runtime_root_ref_payload,
    runtime_root_marker_path, runtime_root_ref_path, runtime_state_placement_payload, RootKind,
    RuntimeStatePlacement, SocketPlacement,
};
use crate::project_identity::{compute_project_id, project_slug};

const SHARED_CACHE_PROVIDERS: &[&str] = &["claude", "codex", "gemini"];
const EXTERNAL_CACHE_PROVIDERS: &[&str] = &["claude", "gemini"];

/// Project-level path layout for a CCB project.
/// Mirrors Python `storage.paths.PathLayout`.
#[derive(Debug, Clone)]
pub struct PathLayout {
    pub project_root: Utf8PathBuf,
    project_id: String,
    runtime_state_placement: RuntimeStatePlacement,
    runtime_state_root: Utf8PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatePayload {
    pub project_root: String,
    pub project_slug: String,
    pub project_id: String,
    pub created_at: String,
}

impl PathLayout {
    pub fn new(project_root: impl Into<Utf8PathBuf>) -> Self {
        let mut root = project_root.into();
        // Try to resolve; fall back to absolute like Python.
        root = if let Ok(resolved) = PathBuf::from(root.as_str()).canonicalize() {
            Utf8PathBuf::from_path_buf(resolved).unwrap_or(root)
        } else if let Ok(absolute) = std::path::absolute(PathBuf::from(root.as_str())) {
            Utf8PathBuf::from_path_buf(absolute).unwrap_or(root)
        } else {
            root
        };

        let project_id = compute_project_id(root.as_str());
        let placement = choose_runtime_state_placement(&root, &project_id, &root.join(".ccb"));
        let state_root = placement.effective_path.clone();

        Self {
            project_root: root,
            project_id,
            runtime_state_placement: placement,
            runtime_state_root: state_root,
        }
    }

    pub fn project_slug(&self) -> String {
        project_slug(self.project_root.as_str())
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn project_socket_key(&self) -> String {
        self.project_id[..12].to_string()
    }

    pub fn runtime_state_placement(&self) -> &RuntimeStatePlacement {
        &self.runtime_state_placement
    }

    pub fn runtime_state_root(&self) -> &Utf8Path {
        &self.runtime_state_root
    }

    // --- Project anchor paths ---

    pub fn project_anchor_dir(&self) -> Utf8PathBuf {
        self.ccb_dir()
    }

    pub fn ccb_dir(&self) -> Utf8PathBuf {
        self.project_root.join(".ccb")
    }

    pub fn config_path(&self) -> Utf8PathBuf {
        self.ccb_dir().join("ccb.config")
    }

    // --- CCBD paths ---

    pub fn ccbd_dir(&self) -> Utf8PathBuf {
        self.runtime_state_root.join("ccbd")
    }

    pub fn ccbd_submissions_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("submissions.jsonl")
    }

    pub fn ccbd_mailboxes_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("mailboxes")
    }

    pub fn ccbd_messages_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("messages")
    }

    pub fn ccbd_messages_path(&self) -> Utf8PathBuf {
        self.ccbd_messages_dir().join("messages.jsonl")
    }

    pub fn ccbd_attempts_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("attempts")
    }

    pub fn ccbd_attempts_path(&self) -> Utf8PathBuf {
        self.ccbd_attempts_dir().join("attempts.jsonl")
    }

    pub fn ccbd_replies_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("replies")
    }

    pub fn ccbd_replies_path(&self) -> Utf8PathBuf {
        self.ccbd_replies_dir().join("replies.jsonl")
    }

    pub fn ccbd_callback_edges_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("callbacks/edges.jsonl")
    }

    pub fn ccbd_leases_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("leases")
    }

    pub fn ccbd_dead_letters_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("dead-letters")
    }

    pub fn ccbd_dead_letters_path(&self) -> Utf8PathBuf {
        self.ccbd_dead_letters_dir().join("dead_letters.jsonl")
    }

    pub fn ccbd_provider_health_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("provider-health")
    }

    // --- Message bureau paths (legacy aliases used by other crates) ---

    pub fn message_bureau_dir(&self) -> Utf8PathBuf {
        self.ccbd_messages_dir()
    }

    pub fn message_store_path(&self) -> Utf8PathBuf {
        self.ccbd_messages_path()
    }

    pub fn attempt_store_path(&self) -> Utf8PathBuf {
        self.ccbd_attempts_path()
    }

    pub fn reply_store_path(&self) -> Utf8PathBuf {
        self.ccbd_replies_path()
    }

    // --- CCBD mount / lifecycle paths ---

    fn project_socket_placement(&self, stem: &str) -> SocketPlacement {
        let preferred_root_kind =
            if matches!(self.runtime_state_placement.root_kind, RootKind::Relocated) {
                RootKind::Runtime
            } else {
                RootKind::Project
            };
        choose_socket_placement(
            &self.ccbd_dir().join(format!("{}.sock", stem)),
            &self.project_socket_key(),
            preferred_root_kind,
        )
    }

    pub fn ccbd_socket_placement(&self) -> SocketPlacement {
        self.project_socket_placement("ccbd")
    }

    pub fn ccbd_socket_path(&self) -> Utf8PathBuf {
        self.ccbd_socket_placement().effective_path
    }

    pub fn ccbd_pid_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("ccbd.pid")
    }

    pub fn ccbd_lifecycle_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("lifecycle.json")
    }

    pub fn ccbd_lease_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("lease.json")
    }

    pub fn ccbd_state_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("state.json")
    }

    pub fn ccbd_project_view_state_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("project-view-state.json")
    }

    pub fn ccbd_start_policy_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("start-policy.json")
    }

    pub fn ccbd_restore_report_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("restore-report.json")
    }

    pub fn ccbd_startup_report_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("startup-report.json")
    }

    pub fn ccbd_shutdown_report_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("shutdown-report.json")
    }

    pub fn ccbd_tmux_socket_placement(&self) -> SocketPlacement {
        self.project_socket_placement("tmux")
    }

    pub fn ccbd_tmux_socket_path(&self) -> Utf8PathBuf {
        self.ccbd_tmux_socket_placement().effective_path
    }

    pub fn ccbd_tmux_session_name(&self) -> String {
        let safe = tmux_safe_name(&self.project_slug(), "project");
        format!("ccb-{}", safe)
    }

    pub fn ccbd_tmux_control_window_name(&self) -> &'static str {
        "__ccb_ctl"
    }

    pub fn ccbd_tmux_workspace_window_name(&self) -> &'static str {
        "ccb"
    }

    // --- CCBD ops paths ---

    pub fn ccbd_supervision_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("supervision.jsonl")
    }

    pub fn ccbd_lifecycle_log_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("lifecycle.jsonl")
    }

    pub fn ccbd_keeper_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("keeper.json")
    }

    pub fn ccbd_shutdown_intent_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("shutdown-intent.json")
    }

    pub fn ccbd_tmux_cleanup_history_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("tmux-cleanup-history.jsonl")
    }

    pub fn ccbd_maintenance_heartbeat_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("maintenance-heartbeat")
    }

    pub fn ccbd_maintenance_heartbeat_schedule_path(&self) -> Utf8PathBuf {
        self.ccbd_maintenance_heartbeat_dir().join("schedule.json")
    }

    pub fn ccbd_maintenance_heartbeat_status_path(&self) -> Utf8PathBuf {
        self.ccbd_maintenance_heartbeat_dir().join("status.json")
    }

    pub fn ccbd_maintenance_heartbeat_runner_path(&self) -> Utf8PathBuf {
        self.ccbd_maintenance_heartbeat_dir().join("runner.json")
    }

    pub fn ccbd_maintenance_heartbeat_lock_path(&self) -> Utf8PathBuf {
        self.ccbd_maintenance_heartbeat_dir().join("lock.json")
    }

    pub fn ccbd_maintenance_heartbeat_activations_path(&self) -> Utf8PathBuf {
        self.ccbd_maintenance_heartbeat_dir()
            .join("activations.jsonl")
    }

    pub fn ccbd_fault_injection_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("fault-injection.json")
    }

    pub fn ccbd_reload_drain_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("reload-drain.json")
    }

    pub fn ccbd_reload_handoff_path(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("reload-handoff.json")
    }

    // --- CCBD artifact paths ---

    pub fn ccbd_artifacts_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("artifacts")
    }

    pub fn ccbd_text_artifacts_dir(&self) -> Utf8PathBuf {
        self.ccbd_artifacts_dir().join("text")
    }

    pub fn ccbd_support_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("support")
    }

    pub fn ccbd_executions_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("executions")
    }

    pub fn ccbd_snapshots_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("snapshots")
    }

    pub fn ccbd_cursors_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("cursors")
    }

    pub fn ccbd_heartbeats_dir(&self) -> Utf8PathBuf {
        self.ccbd_dir().join("heartbeats")
    }

    // --- Agent paths ---

    pub fn agents_dir(&self) -> Utf8PathBuf {
        self.runtime_state_root.join("agents")
    }

    pub fn provider_profiles_dir(&self) -> Utf8PathBuf {
        self.ccb_dir().join("provider-profiles")
    }

    pub fn agent_dir(&self, agent_name: &str) -> Utf8PathBuf {
        self.agents_dir()
            .join(normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase()))
    }

    pub fn agent_anchor_dir(&self, agent_name: &str) -> Utf8PathBuf {
        self.ccb_dir()
            .join("agents")
            .join(normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase()))
    }

    pub fn agent_private_memory_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_anchor_dir(agent_name).join("memory.md")
    }

    pub fn agent_spec_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("agent.json")
    }

    pub fn agent_runtime_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("runtime.json")
    }

    pub fn agent_helper_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("helper.json")
    }

    pub fn agent_provider_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("provider.json")
    }

    pub fn agent_restore_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("restore.json")
    }

    pub fn agent_jobs_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("jobs.jsonl")
    }

    pub fn job_store_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_jobs_path(agent_name)
    }

    pub fn agent_events_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("events.jsonl")
    }

    pub fn agent_provider_runtime_dir(&self, agent_name: &str, provider: &str) -> Utf8PathBuf {
        let normalized = provider.trim().to_lowercase();
        self.agent_dir(agent_name)
            .join("provider-runtime")
            .join(normalized)
    }

    pub fn agent_provider_state_dir(&self, agent_name: &str, provider: &str) -> Utf8PathBuf {
        let normalized = provider.trim().to_lowercase();
        self.agent_dir(agent_name)
            .join("provider-state")
            .join(normalized)
    }

    pub fn agent_logs_dir(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_dir(agent_name).join("logs")
    }

    // --- Agent mailbox paths ---

    pub fn agent_mailbox_dir(&self, agent_name: &str) -> Utf8PathBuf {
        self.ccbd_mailboxes_dir()
            .join(normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase()))
    }

    pub fn agent_mailbox_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_mailbox_dir(agent_name).join("mailbox.json")
    }

    pub fn agent_inbox_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_mailbox_dir(agent_name).join("inbox.jsonl")
    }

    pub fn agent_outbox_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.agent_mailbox_dir(agent_name).join("outbox.jsonl")
    }

    pub fn mailbox_lease_path(&self, agent_name: &str) -> Utf8PathBuf {
        self.ccbd_leases_dir().join(format!(
            "{}.json",
            normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase())
        ))
    }

    // --- Workspace paths ---

    pub fn workspaces_dir(&self) -> Utf8PathBuf {
        self.ccb_dir().join("workspaces")
    }

    pub fn workspace_path(&self, agent_name: &str, workspace_root: Option<&str>) -> Utf8PathBuf {
        let normalized =
            normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase());
        if let Some(root) = workspace_root {
            Utf8PathBuf::from(expand_user_path(root))
                .join(self.project_slug())
                .join(normalized)
        } else {
            self.workspaces_dir().join(normalized)
        }
    }

    pub fn workspace_group_path(&self, group_name: &str) -> Utf8PathBuf {
        self.workspaces_dir()
            .join("groups")
            .join(normalize_agent_name(group_name).unwrap_or_else(|_| group_name.to_lowercase()))
    }

    pub fn workspace_binding_path(
        &self,
        agent_name: &str,
        workspace_root: Option<&str>,
    ) -> Utf8PathBuf {
        self.workspace_path(agent_name, workspace_root)
            .join(".ccb-workspace.json")
    }

    pub fn workspace_group_binding_path(&self, group_name: &str) -> Utf8PathBuf {
        self.workspace_group_path(group_name)
            .join(".ccb-workspace.json")
    }

    // --- Target paths ---

    pub fn target_dir(&self, target_kind: &str, target_name: &str) -> crate::Result<Utf8PathBuf> {
        let segment = crate::path_helpers::target_segment(target_kind, target_name)?;
        if target_kind.trim().to_lowercase() == "agent" {
            Ok(self.agent_dir(&segment))
        } else {
            Ok(self.ccbd_dir().join("targets").join(segment))
        }
    }

    pub fn target_jobs_path(
        &self,
        target_kind: &str,
        target_name: &str,
    ) -> crate::Result<Utf8PathBuf> {
        Ok(self
            .target_dir(target_kind, target_name)?
            .join("jobs.jsonl"))
    }

    pub fn target_events_path(
        &self,
        target_kind: &str,
        target_name: &str,
    ) -> crate::Result<Utf8PathBuf> {
        Ok(self
            .target_dir(target_kind, target_name)?
            .join("events.jsonl"))
    }

    pub fn snapshot_path(&self, job_id: &str) -> Utf8PathBuf {
        self.ccbd_snapshots_dir().join(format!("{}.json", job_id))
    }

    pub fn cursor_path(&self, job_id: &str) -> Utf8PathBuf {
        self.ccbd_cursors_dir().join(format!("{}.json", job_id))
    }

    pub fn execution_state_path(&self, job_id: &str) -> Utf8PathBuf {
        self.ccbd_executions_dir().join(format!("{}.json", job_id))
    }

    pub fn heartbeat_subject_dir(&self, subject_kind: &str) -> crate::Result<Utf8PathBuf> {
        Ok(self
            .ccbd_heartbeats_dir()
            .join(normalized_segment(subject_kind, "subject_kind")?))
    }

    pub fn heartbeat_subject_path(
        &self,
        subject_kind: &str,
        subject_id: &str,
    ) -> crate::Result<Utf8PathBuf> {
        let normalized_id = normalized_segment(subject_id, "subject_id")?;
        Ok(self
            .heartbeat_subject_dir(subject_kind)?
            .join(format!("{}.json", normalized_id)))
    }

    pub fn provider_health_path(&self, job_id: &str) -> Utf8PathBuf {
        self.ccbd_provider_health_dir()
            .join(format!("{}.jsonl", job_id.trim()))
    }

    pub fn support_bundle_path(&self, bundle_id: &str) -> crate::Result<Utf8PathBuf> {
        let normalized = normalized_segment(bundle_id, "bundle_id")?;
        Ok(self
            .ccbd_support_dir()
            .join(format!("{}.tar.gz", normalized)))
    }

    // --- Memory paths ---

    pub fn project_memory_path(&self) -> Utf8PathBuf {
        self.ccb_dir().join("ccb_memory.md")
    }

    pub fn memory_seed_path(&self) -> Utf8PathBuf {
        self.runtime_state_root.join("state/memory.seed.json")
    }

    pub fn runtime_memory_dir(&self) -> Utf8PathBuf {
        self.runtime_state_root.join("runtime/memory")
    }

    pub fn runtime_memory_bundle_path(&self, agent_name: &str) -> Utf8PathBuf {
        let normalized =
            normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase());
        self.runtime_memory_dir().join(format!("{}.md", normalized))
    }

    // --- Shared cache ---

    pub fn shared_cache_dir(&self) -> Utf8PathBuf {
        self.runtime_state_root.join("shared-cache")
    }

    pub fn provider_shared_cache_dir(&self, provider: &str) -> crate::Result<Utf8PathBuf> {
        let normalized = normalized_segment(provider, "provider")?;
        let original = provider.trim().to_lowercase();
        if normalized != original || !SHARED_CACHE_PROVIDERS.contains(&normalized.as_str()) {
            return Err(crate::StorageError::Corrupt(format!(
                "provider must be one of: {}",
                SHARED_CACHE_PROVIDERS.join(", ")
            )));
        }
        Ok(self.shared_cache_dir().join(normalized))
    }

    pub fn ensure_provider_shared_cache_dir(
        &self,
        provider: &str,
        created_at: Option<&str>,
    ) -> crate::Result<Utf8PathBuf> {
        let placement = self.runtime_state_placement();
        if placement.filesystem_hint.as_deref() == Some("wsl_drvfs")
            && !matches!(placement.root_kind, RootKind::Relocated)
        {
            return Err(crate::StorageError::Corrupt(
                "shared cache requires relocated runtime state for WSL drvfs project anchors"
                    .into(),
            ));
        }
        let cache_dir = self.provider_shared_cache_dir(provider)?;
        let timestamp = created_at.map(|s| s.to_string()).unwrap_or_else(utc_now);
        self.ensure_runtime_state_root(Some(&timestamp))?;
        fs::create_dir_all(&cache_dir)?;
        let manifest_path = cache_dir.join("MANIFEST.json");
        if !manifest_path.exists() {
            atomic_write_json(
                &manifest_path,
                &serde_json::json!({
                    "schema_version": 1,
                    "record_type": "ccb_shared_cache_manifest",
                    "provider": cache_dir.file_name().unwrap_or("unknown"),
                    "project_id": self.project_id,
                    "runtime_state_root": self.runtime_state_root.as_str(),
                    "created_at": timestamp,
                    "entries": [],
                }),
            )?;
        }
        Ok(cache_dir)
    }

    pub fn external_provider_cache_root(&self) -> Utf8PathBuf {
        let root = user_cache_home();
        root.join("ccb/projects")
            .join(&self.project_id[..16])
            .join("provider-cache")
    }

    pub fn provider_external_cache_dir(&self, provider: &str) -> crate::Result<Utf8PathBuf> {
        let normalized = normalized_segment(provider, "provider")?;
        let original = provider.trim().to_lowercase();
        if normalized != original || !EXTERNAL_CACHE_PROVIDERS.contains(&normalized.as_str()) {
            return Err(crate::StorageError::Corrupt(format!(
                "provider must be one of: {}",
                EXTERNAL_CACHE_PROVIDERS.join(", ")
            )));
        }
        Ok(self.external_provider_cache_root().join(normalized))
    }

    pub fn ensure_provider_external_cache_dir(
        &self,
        provider: &str,
        created_at: Option<&str>,
    ) -> crate::Result<Utf8PathBuf> {
        let cache_dir = self.provider_external_cache_dir(provider)?;
        let timestamp = created_at.map(|s| s.to_string()).unwrap_or_else(utc_now);
        fs::create_dir_all(&cache_dir)?;
        let manifest_path = cache_dir.join("MANIFEST.json");
        if !manifest_path.exists() {
            atomic_write_json(
                &manifest_path,
                &serde_json::json!({
                    "schema_version": 1,
                    "record_type": "ccb_external_provider_cache_manifest",
                    "provider": cache_dir.file_name().unwrap_or("unknown"),
                    "project_id": self.project_id,
                    "project_root": self.project_root.as_str(),
                    "created_at": timestamp,
                    "entries": [],
                }),
            )?;
        }
        Ok(cache_dir)
    }

    // --- Runtime root marker / ref ---

    pub fn runtime_root_marker_path(&self) -> Utf8PathBuf {
        runtime_root_marker_path(&self.runtime_state_root)
    }

    pub fn runtime_root_ref_path(&self) -> Utf8PathBuf {
        runtime_root_ref_path(&self.ccb_dir())
    }

    pub fn runtime_marker_status(&self) -> String {
        if self.runtime_state_placement.is_project_scoped() {
            return "not_required".into();
        }
        match self.validate_runtime_root_marker(false) {
            Ok(()) => match self.validate_runtime_root_ref(true) {
                Ok(()) => "ok".into(),
                Err(_) => "mismatch".into(),
            },
            Err(crate::StorageError::NotFound(_)) => "missing".into(),
            Err(_) => "mismatch".into(),
        }
    }

    pub fn ensure_runtime_state_root(&self, created_at: Option<&str>) -> std::io::Result<()> {
        if self.runtime_state_placement.is_project_scoped() {
            return Ok(());
        }
        fs::create_dir_all(self.ccb_dir())?;
        fs::create_dir_all(&self.runtime_state_root)?;
        let timestamp = created_at.map(|s| s.to_string()).unwrap_or_else(utc_now);
        if let Err(e) = self.validate_runtime_root_marker(true) {
            if !e.to_string().contains("No such file") {
                return Err(std::io::Error::other(e));
            }
        }
        if let Err(e) = self.validate_runtime_root_ref(true) {
            if !e.to_string().contains("No such file") {
                return Err(std::io::Error::other(e));
            }
        }
        atomic_write_json(
            &self.runtime_root_marker_path(),
            &self.runtime_root_marker_payload(&timestamp),
        )
        .map_err(std::io::Error::other)?;
        atomic_write_json(
            &self.runtime_root_ref_path(),
            &self.runtime_root_ref_payload(&timestamp),
        )
        .map_err(std::io::Error::other)?;
        Ok(())
    }

    pub fn runtime_state_payload(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut payload = runtime_state_placement_payload(&self.runtime_state_placement);
        payload.insert(
            "runtime_marker_status".into(),
            self.runtime_marker_status().into(),
        );
        payload.insert(
            "runtime_root_marker_path".into(),
            self.runtime_root_marker_path().as_str().into(),
        );
        payload.insert(
            "runtime_root_ref_path".into(),
            self.runtime_root_ref_path().as_str().into(),
        );
        payload
    }

    fn runtime_root_marker_payload(&self, created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "record_type": "ccb_runtime_root",
            "project_id": self.project_id,
            "project_root": self.project_root.as_str(),
            "anchor_path": self.ccb_dir().as_str(),
            "runtime_root_path": self.runtime_state_root.as_str(),
            "created_at": created_at,
        })
    }

    fn runtime_root_ref_payload(&self, created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "record_type": "ccb_runtime_root_ref",
            "project_id": self.project_id,
            "runtime_state_root": self.runtime_state_root.as_str(),
            "created_at": created_at,
        })
    }

    fn validate_runtime_root_marker(&self, allow_missing: bool) -> crate::Result<()> {
        let payload = read_runtime_root_marker_payload(&self.runtime_root_marker_path());
        if payload.is_none() {
            if allow_missing && !self.runtime_root_marker_path().exists() {
                return Ok(());
            }
            if !self.runtime_root_marker_path().exists() {
                return Err(crate::StorageError::NotFound(
                    self.runtime_root_marker_path().to_string(),
                ));
            }
            return Err(crate::StorageError::Corrupt(format!(
                "{} is invalid",
                self.runtime_root_marker_path()
            )));
        }
        let payload = payload.unwrap();
        let ccb_dir = self.ccb_dir();
        let expected = [
            ("project_id", self.project_id.as_str()),
            ("project_root", self.project_root.as_str()),
            ("anchor_path", ccb_dir.as_str()),
            ("runtime_root_path", self.runtime_state_root.as_str()),
        ];
        for (key, value) in expected {
            let recorded = payload
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if recorded != value {
                return Err(crate::StorageError::Corrupt(format!(
                    "{} field {} mismatch: expected {}, found {}",
                    self.runtime_root_marker_path(),
                    key,
                    value,
                    if recorded.is_empty() {
                        "<missing>"
                    } else {
                        recorded
                    }
                )));
            }
        }
        Ok(())
    }

    fn validate_runtime_root_ref(&self, allow_missing: bool) -> crate::Result<()> {
        let payload = read_runtime_root_ref_payload(&self.ccb_dir(), Some(&self.project_id));
        if payload.is_none() {
            if allow_missing && !self.runtime_root_ref_path().exists() {
                return Ok(());
            }
            if !self.runtime_root_ref_path().exists() {
                return Err(crate::StorageError::NotFound(
                    self.runtime_root_ref_path().to_string(),
                ));
            }
            return Err(crate::StorageError::Corrupt(format!(
                "{} is invalid",
                self.runtime_root_ref_path()
            )));
        }
        let payload = payload.unwrap();
        let expected = [
            ("project_id", self.project_id.as_str()),
            ("runtime_state_root", self.runtime_state_root.as_str()),
        ];
        for (key, value) in expected {
            let recorded = payload
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if recorded != value {
                return Err(crate::StorageError::Corrupt(format!(
                    "{} field {} mismatch: expected {}, found {}",
                    self.runtime_root_ref_path(),
                    key,
                    value,
                    if recorded.is_empty() {
                        "<missing>"
                    } else {
                        recorded
                    }
                )));
            }
        }
        Ok(())
    }
}

fn tmux_safe_name(value: &str, fallback: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches(&['_', '-'][..]);
    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized.to_string()
    }
}

fn user_cache_home() -> Utf8PathBuf {
    if let Ok(raw) = std::env::var("XDG_CACHE_HOME") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Ok(path) = Utf8PathBuf::from_path_buf(PathBuf::from(expand_user_path(trimmed))) {
                return path;
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    Utf8PathBuf::from(format!("{}/.cache", expand_user_path(&home)))
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

fn utc_now() -> String {
    chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        .replace("+00:00", "Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_slug() {
        let layout = PathLayout::new("/home/user/my-project");
        assert!(layout.project_slug().starts_with("my-project-"));
    }

    #[test]
    fn test_project_id_deterministic() {
        let a = PathLayout::new("/home/user/project");
        let b = PathLayout::new("/home/user/project");
        assert_eq!(a.project_id, b.project_id);
    }

    #[test]
    fn test_socket_key_length() {
        let layout = PathLayout::new("/home/user/project");
        assert_eq!(layout.project_socket_key().len(), 12);
    }

    #[test]
    fn test_ccb_dir_structure() {
        let layout = PathLayout::new("/project");
        assert_eq!(layout.ccb_dir(), Utf8PathBuf::from("/project/.ccb"));
        assert_eq!(layout.ccbd_dir(), Utf8PathBuf::from("/project/.ccb/ccbd"));
    }

    #[test]
    fn test_agent_mailbox_path() {
        let layout = PathLayout::new("/project");
        assert_eq!(
            layout.agent_mailbox_path("Agent1"),
            Utf8PathBuf::from("/project/.ccb/ccbd/mailboxes/agent1/mailbox.json")
        );
    }

    #[test]
    fn test_provider_shared_cache_dir() {
        let layout = PathLayout::new("/project");
        assert_eq!(
            layout.provider_shared_cache_dir("claude").unwrap(),
            Utf8PathBuf::from("/project/.ccb/shared-cache/claude")
        );
    }

    #[test]
    fn test_rejects_noncanonical_shared_cache_provider() {
        let layout = PathLayout::new("/project");
        assert!(layout.provider_shared_cache_dir("Claude Code").is_err());
    }
}
