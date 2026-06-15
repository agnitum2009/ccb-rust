//! Mirrors Python `lib/ccbd/supervision/recovery_transitions.py`.
//! 1:1 file alignment stub.

use std::collections::HashMap;

const SUCCESS_RUNTIME_HEALTHS: &[&str] = &["healthy", "restored"];

/// Start recovery process for an agent
pub fn start_recovery(
    ctx: &dyn RecoveryContext,
    attempted_at: &str,
    prior_health: &str,
) -> Result<AgentRuntime, String> {
    let runtime = ctx.runtime();
    let recovering = ctx.upsert_if_changed(
        runtime,
        "recovering",
        attempted_at,
        "recovering",
        None,
        None,
    )?;
    ctx.append_recovery_event(
        "recover_started",
        attempted_at,
        &recovering,
        prior_health,
        prior_health,
        None,
    );
    Ok(recovering)
}

/// Attempt recovery action for an agent
pub fn attempt_recovery_action(
    ctx: &dyn RecoveryContext,
    recovering: &AgentRuntime,
) -> Result<(Option<AgentRuntime>, Option<String>), String> {
    if ctx.should_reflow_project_namespace(recovering, None) {
        ctx.remount_project(&format!("pane_recovery:{}", ctx.agent_name()));
        return Ok((ctx.registry_get(ctx.agent_name()), None));
    }

    let refreshed = ctx.runtime_service_refresh_provider_binding(ctx.agent_name(), true)?;
    if refreshed.is_none() {
        return Ok((None, None));
    }

    if ctx.should_reflow_project_namespace(recovering, refreshed.as_ref()) {
        ctx.remount_project(&format!("pane_recovery:{}", ctx.agent_name()));
        return Ok((ctx.registry_get(ctx.agent_name()), None));
    }

    Ok((refreshed, None))
}

/// Mark recovery as missing (runtime disappeared after recover)
pub fn mark_recovery_missing(
    ctx: &dyn RecoveryContext,
    recovering: &AgentRuntime,
    attempted_at: &str,
    restart_count: u32,
    prior_health: &str,
) -> Result<String, String> {
    let failed = ctx.upsert_if_changed(
        recovering.clone(),
        "degraded",
        attempted_at,
        "degraded",
        Some(restart_count),
        Some("runtime-missing-after-recover"),
    )?;

    let mut details = HashMap::new();
    details.insert(
        "reason".to_string(),
        "runtime-missing-after-recover".to_string(),
    );

    ctx.append_recovery_event(
        "recover_failed",
        attempted_at,
        &failed,
        prior_health,
        "unmounted",
        Some(&details),
    );
    Ok("unmounted".to_string())
}

/// Mark recovery as succeeded
pub fn mark_recovery_succeeded(
    ctx: &dyn RecoveryContext,
    refreshed: &AgentRuntime,
    attempted_at: &str,
    restart_count: u32,
    prior_health: &str,
    next_health: &str,
) -> Result<String, String> {
    let stabilized = ctx.upsert_if_changed(
        refreshed.clone(),
        "steady",
        attempted_at,
        &refreshed.lifecycle_state(),
        Some(restart_count),
        None,
    )?;

    let mut details = HashMap::new();
    details.insert(
        "restart_count".to_string(),
        stabilized.restart_count().to_string(),
    );

    ctx.append_recovery_event(
        "recover_succeeded",
        attempted_at,
        &stabilized,
        prior_health,
        next_health,
        Some(&details),
    );
    Ok(stabilized.health())
}

/// Mark recovery as failed
pub fn mark_recovery_failed(
    ctx: &dyn RecoveryContext,
    refreshed: &AgentRuntime,
    attempted_at: &str,
    restart_count: u32,
    prior_health: &str,
    next_health: &str,
    failure_reason: Option<&str>,
) -> Result<String, String> {
    let reason = failure_reason
        .or(Some(next_health))
        .or(Some(prior_health))
        .unwrap_or("recover-failed");
    let failure_runtime = ctx.upsert_if_changed(
        refreshed.clone(),
        "degraded",
        attempted_at,
        "degraded",
        Some(restart_count),
        Some(reason),
    )?;

    let mut details = HashMap::new();
    details.insert(
        "reason".to_string(),
        failure_runtime
            .last_failure_reason()
            .unwrap_or("recover-failed")
            .to_string(),
    );

    ctx.append_recovery_event(
        "recover_failed",
        attempted_at,
        &failure_runtime,
        prior_health,
        next_health,
        Some(&details),
    );
    Ok(failure_runtime.health())
}

pub trait RecoveryContext {
    fn runtime(&self) -> AgentRuntime;
    fn agent_name(&self) -> &str;
    fn upsert_if_changed(
        &self,
        runtime: AgentRuntime,
        reconcile_state: &str,
        last_reconcile_at: &str,
        lifecycle_state: &str,
        restart_count: Option<u32>,
        failure_reason: Option<&str>,
    ) -> Result<AgentRuntime, String>;
    fn append_recovery_event(
        &self,
        event_kind: &str,
        occurred_at: &str,
        runtime: &AgentRuntime,
        prior_health: &str,
        result_health: &str,
        details: Option<&HashMap<String, String>>,
    );
    fn should_reflow_project_namespace(
        &self,
        runtime: &AgentRuntime,
        recovered: Option<&AgentRuntime>,
    ) -> bool;
    fn remount_project(&self, mount_key: &str);
    fn registry_get(&self, agent_name: &str) -> Option<AgentRuntime>;
    fn runtime_service_refresh_provider_binding(
        &self,
        agent_name: &str,
        recover: bool,
    ) -> Result<Option<AgentRuntime>, String>;
}

#[derive(Debug, Clone)]
pub struct AgentRuntime {
    pub health: String,
    pub lifecycle_state: String,
    pub restart_count: u32,
    pub last_failure_reason: Option<String>,
}

impl AgentRuntime {
    pub fn health(&self) -> String {
        self.health.clone()
    }
    pub fn lifecycle_state(&self) -> String {
        self.lifecycle_state.clone()
    }
    pub fn restart_count(&self) -> u32 {
        self.restart_count
    }
    pub fn last_failure_reason(&self) -> Option<&str> {
        self.last_failure_reason.as_deref()
    }
}
