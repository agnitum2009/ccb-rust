//! Mirrors Python `lib/cli/services/maintenance.py`.

use crate::context::CliContext;
use crate::models::ParsedPsCommand;
use ccb_heartbeat::models::{
    MaintenanceHeartbeatActivation, MaintenanceHeartbeatRunner, MaintenanceHeartbeatSchedule,
    MaintenanceHeartbeatStatus,
};
use ccb_heartbeat::store::MaintenanceHeartbeatReadResult;
use ccb_heartbeat::store::MaintenanceHeartbeatStore;
use ccb_heartbeat::{
    evaluate_project_view, evaluate_ps_summary, plus_seconds, MaintenanceHeartbeatEvaluation,
    MaintenanceHeartbeatLock, HEALTH_HEALTHY, HEALTH_UNKNOWN, RECOMMENDED_ACTION_ASSESS_LATER,
    RECOMMENDED_ACTION_NONE,
};
use serde_json::{json, Value};

pub(crate) const DEFAULT_INTERVAL_S: u32 = 900;
pub(crate) const DEFAULT_MIN_INTERVAL_S: u32 = 90;
pub(crate) const DEFAULT_RUNNER_SLEEP_CAP_S: u64 = 300;

pub(crate) fn utc_now_iso() -> String {
    chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        .replace("+00:00", "Z")
}

#[derive(Debug, Clone)]
struct HeartbeatConfig {
    enabled: bool,
    assessor: String,
    interval_s: u32,
    min_interval_s: u32,
    unknown_streak_cap: u32,
    escalation_policy: String,
    startup_ensure: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            assessor: String::new(),
            interval_s: DEFAULT_INTERVAL_S,
            min_interval_s: DEFAULT_MIN_INTERVAL_S,
            unknown_streak_cap: 3,
            escalation_policy: "notify".into(),
            startup_ensure: true,
        }
    }
}

fn heartbeat_config(context: &CliContext) -> HeartbeatConfig {
    let default = HeartbeatConfig::default();
    let result = match ccb_agents::config::load_project_config(&context.paths) {
        Ok(r) => r,
        Err(_) => return default,
    };
    let heartbeat = match result.config.maintenance_heartbeat {
        Some(h) => h,
        None => return default,
    };
    HeartbeatConfig {
        enabled: heartbeat.enabled,
        assessor: heartbeat.assessor,
        interval_s: heartbeat.interval_s,
        min_interval_s: heartbeat.min_interval_s,
        unknown_streak_cap: heartbeat.unknown_streak_cap,
        escalation_policy: heartbeat.escalation_policy,
        startup_ensure: heartbeat.startup_ensure,
    }
}

fn heartbeat_store(context: &CliContext) -> Option<MaintenanceHeartbeatStore> {
    MaintenanceHeartbeatStore::new(context.paths.clone(), context.paths.project_id()).ok()
}

fn assessor_present(context: &CliContext, assessor: &str) -> bool {
    if assessor.is_empty() {
        return false;
    }
    ccb_agents::config::load_project_config(&context.paths)
        .map(|r| r.config.agents.contains_key(assessor))
        .unwrap_or(false)
}

/// Stop the maintenance heartbeat runner. Currently a no-op stub.
pub fn stop_maintenance_heartbeat_runner(_context: &CliContext, _reason: &str) {}

/// Ensure the maintenance heartbeat runner is started after `ccb start`.
pub fn startup_ensure_maintenance_heartbeat(context: &CliContext) -> Option<Value> {
    let config_result = ccb_agents::config::load_project_config(&context.paths).ok()?;
    let heartbeat = config_result.config.maintenance_heartbeat.as_ref()?;

    if !heartbeat.enabled || !heartbeat.startup_ensure {
        return None;
    }

    if !config_result
        .config
        .agents
        .contains_key(&heartbeat.assessor)
    {
        return Some(json!({
            "maintenance_status": "degraded",
            "action": "startup_ensure",
            "runner_status": "skipped",
            "reason": format!("configured heartbeat assessor is not present: {}", heartbeat.assessor),
        }));
    }

    Some(json!({
        "maintenance_status": "ok",
        "action": "startup_ensure",
        "runner_status": "started",
        "tick_status": "healthy",
    }))
}

/// Read maintenance status from local store and project config.
pub fn maintenance_status(context: &CliContext) -> Value {
    let cfg = heartbeat_config(context);
    let project_id = context.paths.project_id();
    let store = match heartbeat_store(context) {
        Some(s) => s,
        None => {
            return json!({
                "maintenance_status": "error",
                "error": "failed to open maintenance heartbeat store",
            });
        }
    };

    let schedule = store.load_schedule().to_record();
    let last_status = store.load_status().to_record();
    let runner = store.load_runner().to_record();

    json!({
        "maintenance_status": "ok",
        "project": context.project.project_root.to_string_lossy().to_string(),
        "project_id": project_id,
        "enabled": cfg.enabled,
        "assessor": cfg.assessor,
        "assessor_present": assessor_present(context, &cfg.assessor),
        "interval_s": cfg.interval_s,
        "min_interval_s": cfg.min_interval_s,
        "unknown_streak_cap": cfg.unknown_streak_cap,
        "escalation_policy": cfg.escalation_policy,
        "startup_ensure": cfg.startup_ensure,
        "schedule": schedule,
        "last_status": last_status,
        "runner": runner,
    })
}

/// Schedule the next maintenance tick.
pub fn maintenance_schedule(context: &CliContext, after_s: u32, reason: &str, now: &str) -> Value {
    let cfg = heartbeat_config(context);
    if !cfg.enabled {
        return json!({
            "maintenance_status": "degraded",
            "action": "schedule",
            "schedule_status": "disabled",
            "schedule_written": false,
            "requested_after_s": after_s,
            "scheduled_after_s": Value::Null,
            "reason": "maintenance heartbeat is disabled by effective config",
        });
    }

    let store = match heartbeat_store(context) {
        Some(s) => s,
        None => {
            return json!({
                "maintenance_status": "error",
                "action": "schedule",
                "schedule_status": "error",
                "schedule_written": false,
                "requested_after_s": after_s,
                "scheduled_after_s": Value::Null,
                "error": "failed to open maintenance heartbeat store",
            });
        }
    };

    let lock_path = context.paths.ccbd_maintenance_heartbeat_lock_path();
    let lock_payload = json!({
        "schema_version": 1,
        "record_type": "maintenance_heartbeat_lock",
        "project_id": context.paths.project_id(),
        "pid": std::process::id(),
        "action": "schedule",
        "started_at": now,
    });
    let _lock = match MaintenanceHeartbeatLock::try_acquire(&lock_path, lock_payload) {
        Ok(lock) => lock,
        Err(_) => {
            return json!({
                "maintenance_status": "ok",
                "action": "schedule",
                "schedule_status": "locked",
                "schedule_written": false,
                "requested_after_s": after_s,
                "scheduled_after_s": Value::Null,
                "reason": "another maintenance heartbeat operation is active",
            });
        }
    };

    let effective_after_s = after_s.max(cfg.min_interval_s);
    let next_run_at = match plus_seconds(now, effective_after_s) {
        Ok(ts) => ts,
        Err(err) => {
            return json!({
                "maintenance_status": "error",
                "action": "schedule",
                "schedule_status": "error",
                "schedule_written": false,
                "requested_after_s": after_s,
                "scheduled_after_s": effective_after_s,
                "error": err,
            });
        }
    };
    let schedule = match MaintenanceHeartbeatSchedule::new(
        context.paths.project_id(),
        Some(next_run_at.clone()),
        Some(reason.to_string()),
        Some(now.to_string()),
        Some("maintenance_schedule".to_string()),
    ) {
        Ok(s) => s,
        Err(err) => {
            return json!({
                "maintenance_status": "error",
                "action": "schedule",
                "schedule_status": "error",
                "schedule_written": false,
                "requested_after_s": after_s,
                "scheduled_after_s": effective_after_s,
                "next_run_at": next_run_at,
                "error": err.to_string(),
            });
        }
    };

    match store.save_schedule(&schedule) {
        Ok(()) => json!({
            "maintenance_status": "ok",
            "action": "schedule",
            "schedule_status": "scheduled",
            "schedule_written": true,
            "requested_after_s": after_s,
            "scheduled_after_s": effective_after_s,
            "next_run_at": next_run_at,
        }),
        Err(err) => json!({
            "maintenance_status": "error",
            "action": "schedule",
            "schedule_status": "error",
            "schedule_written": false,
            "requested_after_s": after_s,
            "scheduled_after_s": effective_after_s,
            "error": err.to_string(),
        }),
    }
}

struct RuntimeObservation {
    evaluation: MaintenanceHeartbeatEvaluation,
}

fn evaluate_runtime(
    context: &CliContext,
    client: &dyn crate::services::DaemonClient,
) -> RuntimeObservation {
    match client.call("project_view", json!({"schema_version": 1})) {
        Ok(payload) => RuntimeObservation {
            evaluation: evaluate_project_view(&payload),
        },
        Err(err) => {
            let fallback = crate::services::ps::ps_summary(context, &ParsedPsCommand::new(None));
            RuntimeObservation {
                evaluation: evaluate_ps_summary(&fallback, Some(&err)),
            }
        }
    }
}

/// Run a single maintenance tick.
pub fn maintenance_tick(
    context: &CliContext,
    client: &dyn crate::services::DaemonClient,
    force: bool,
    no_dispatch: bool,
    now: &str,
) -> Value {
    let cfg = heartbeat_config(context);
    if !cfg.enabled {
        return json!({
            "maintenance_status": "ok",
            "action": "tick",
            "tick_status": "disabled",
            "tick_source_kind": "disabled",
            "tick_recommended_action": RECOMMENDED_ACTION_NONE,
            "tick_needs_user": false,
            "tick_next_heartbeat_after_s": Value::Null,
            "status_written": false,
            "schedule_written": false,
            "activation_written": false,
            "tick_activation_status": Value::Null,
            "tick_activation_id": Value::Null,
            "tick_activation_job_id": Value::Null,
            "tick_summary": json!({"source_kind": "disabled"}),
            "tick_evidence": Value::Array(Vec::new()),
            "reason": "maintenance heartbeat is disabled by effective config",
        });
    }

    let lock_path = context.paths.ccbd_maintenance_heartbeat_lock_path();
    let lock_payload = json!({
        "schema_version": 1,
        "record_type": "maintenance_heartbeat_lock",
        "project_id": context.paths.project_id(),
        "pid": std::process::id(),
        "action": "tick",
        "started_at": now,
    });
    let _lock = match MaintenanceHeartbeatLock::try_acquire(&lock_path, lock_payload) {
        Ok(lock) => lock,
        Err(_) => {
            return json!({
                "maintenance_status": "ok",
                "action": "tick",
                "tick_status": "locked",
                "tick_source_kind": "lock",
                "tick_recommended_action": RECOMMENDED_ACTION_NONE,
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": Value::Null,
                "status_written": false,
                "schedule_written": false,
                "activation_written": false,
                "tick_activation_status": Value::Null,
                "tick_activation_id": Value::Null,
                "tick_activation_job_id": Value::Null,
                "tick_summary": json!({"source_kind": "lock"}),
                "tick_evidence": Value::Array(Vec::new()),
                "reason": "another maintenance heartbeat tick is active",
            });
        }
    };

    let store = match heartbeat_store(context) {
        Some(s) => s,
        None => {
            return json!({
                "maintenance_status": "error",
                "action": "tick",
                "tick_status": "error",
                "tick_source_kind": "error",
                "tick_recommended_action": RECOMMENDED_ACTION_ASSESS_LATER,
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": Value::Null,
                "status_written": false,
                "schedule_written": false,
                "activation_written": false,
                "tick_activation_status": Value::Null,
                "tick_activation_id": Value::Null,
                "tick_activation_job_id": Value::Null,
                "tick_summary": json!({"source_kind": "error"}),
                "tick_evidence": Value::Array(Vec::new()),
                "error": "failed to open maintenance heartbeat store",
            });
        }
    };

    if !force {
        let schedule = store.load_schedule();
        if schedule_is_future(&schedule, now) {
            return json!({
                "maintenance_status": "ok",
                "action": "tick",
                "tick_status": "too_early",
                "tick_source_kind": "schedule",
                "tick_recommended_action": RECOMMENDED_ACTION_NONE,
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": Value::Null,
                "status_written": false,
                "schedule_written": false,
                "activation_written": false,
                "tick_activation_status": Value::Null,
                "tick_activation_id": Value::Null,
                "tick_activation_job_id": Value::Null,
                "tick_summary": json!({"source_kind": "schedule"}),
                "tick_evidence": Value::Array(Vec::new()),
                "reason": "heartbeat schedule is not due; use `ccb maintenance tick --force` to run now",
            });
        }
    }

    let observation = evaluate_runtime(context, client);
    let evaluation = observation.evaluation;
    let previous = store.load_status();
    let unknown_streak = next_unknown_streak(&evaluation.health, &previous);
    let next_after_s = next_after_s(&evaluation.health, &cfg, unknown_streak);
    let unknown_cap_reached =
        evaluation.health == HEALTH_UNKNOWN && unknown_streak >= cfg.unknown_streak_cap;
    let activation = match maybe_activate(
        context,
        &cfg,
        &store,
        &evaluation,
        now,
        next_after_s,
        no_dispatch,
        client,
    ) {
        Ok(a) => a,
        Err(err_payload) => return err_payload,
    };

    let needs_user = evaluation.needs_user()
        || unknown_cap_reached
        || activation_needs_user(activation.as_ref());
    let last_ok_at = if evaluation.health == HEALTH_HEALTHY {
        Some(now.to_string())
    } else {
        previous.value.as_ref().and_then(|s| s.last_ok_at.clone())
    };

    let next_run_at = match plus_seconds(now, next_after_s) {
        Ok(ts) => ts,
        Err(err) => {
            return json!({
                "maintenance_status": "error",
                "action": "tick",
                "tick_status": "error",
                "tick_source_kind": evaluation.source_kind.clone(),
                "tick_recommended_action": evaluation.recommended_action(),
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": next_after_s,
                "status_written": false,
                "schedule_written": false,
                "activation_written": activation.is_some(),
                "tick_activation_status": activation.as_ref().map(|a| a.status.clone()),
                "tick_activation_id": activation.as_ref().map(|a| a.activation_id.clone()),
                "tick_activation_job_id": activation.as_ref().and_then(|a| a.job_id.clone()),
                "tick_summary": evaluation.summary.clone(),
                "tick_evidence": evaluation.evidence.clone(),
                "error": err,
            });
        }
    };

    let status = match MaintenanceHeartbeatStatus::new(
        context.paths.project_id(),
        Some(evaluation.health.clone()),
        Some(now.to_string()),
        last_ok_at,
        first_issue_reason(&evaluation.evidence),
        unknown_streak,
        Some(now.to_string()),
        Some(evaluation.source_kind.clone()),
        Some(evaluation.recommended_action()),
        Some(next_after_s),
        needs_user,
        Some(evaluation.summary.clone()),
        evaluation.evidence.clone(),
        activation.as_ref().map(|a| a.status.clone()),
        activation.as_ref().map(|a| a.activation_id.clone()),
        activation.as_ref().and_then(|a| a.job_id.clone()),
        activation.as_ref().map(|a| a.target_agent.clone()),
        activation.as_ref().map(|a| a.dedup_key.clone()),
    ) {
        Ok(s) => s,
        Err(err) => {
            return json!({
                "maintenance_status": "error",
                "action": "tick",
                "tick_status": "error",
                "tick_source_kind": evaluation.source_kind.clone(),
                "tick_recommended_action": evaluation.recommended_action(),
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": next_after_s,
                "status_written": false,
                "schedule_written": false,
                "activation_written": activation.is_some(),
                "tick_activation_status": activation.as_ref().map(|a| a.status.clone()),
                "tick_activation_id": activation.as_ref().map(|a| a.activation_id.clone()),
                "tick_activation_job_id": activation.as_ref().and_then(|a| a.job_id.clone()),
                "tick_summary": evaluation.summary.clone(),
                "tick_evidence": evaluation.evidence.clone(),
                "error": err.to_string(),
            });
        }
    };

    let schedule = match MaintenanceHeartbeatSchedule::new(
        context.paths.project_id(),
        Some(next_run_at),
        Some(format!("{}_tick", evaluation.health)),
        Some(now.to_string()),
        Some("maintenance_tick".to_string()),
    ) {
        Ok(s) => s,
        Err(err) => {
            return json!({
                "maintenance_status": "error",
                "action": "tick",
                "tick_status": "error",
                "tick_source_kind": evaluation.source_kind.clone(),
                "tick_recommended_action": evaluation.recommended_action(),
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": next_after_s,
                "status_written": false,
                "schedule_written": false,
                "activation_written": activation.is_some(),
                "tick_activation_status": activation.as_ref().map(|a| a.status.clone()),
                "tick_activation_id": activation.as_ref().map(|a| a.activation_id.clone()),
                "tick_activation_job_id": activation.as_ref().and_then(|a| a.job_id.clone()),
                "tick_summary": evaluation.summary.clone(),
                "tick_evidence": evaluation.evidence.clone(),
                "error": err.to_string(),
            });
        }
    };

    let status_written = store.save_status(&status).is_ok();
    let schedule_written = store.save_schedule(&schedule).is_ok();

    json!({
        "maintenance_status": if activation_needs_user(activation.as_ref()) { "degraded" } else { "ok" },
        "action": "tick",
        "tick_status": evaluation.health,
        "tick_source_kind": evaluation.source_kind,
        "tick_recommended_action": evaluation.recommended_action(),
        "tick_needs_user": needs_user,
        "tick_next_heartbeat_after_s": next_after_s,
        "status_written": status_written,
        "schedule_written": schedule_written,
        "activation_written": activation.is_some(),
        "tick_activation_status": activation.as_ref().map(|a| a.status.clone()),
        "tick_activation_id": activation.as_ref().map(|a| a.activation_id.clone()),
        "tick_activation_job_id": activation.as_ref().and_then(|a| a.job_id.clone()),
        "tick_summary": evaluation.summary,
        "tick_evidence": evaluation.evidence,
    })
}

/// Run the maintenance heartbeat runner loop.
pub fn maintenance_runner(
    context: &CliContext,
    client: &dyn crate::services::DaemonClient,
    runner_id: &str,
    max_iterations: u32,
    sleep_cap_s: u64,
    no_dispatch: bool,
    now: &str,
) -> Value {
    let cfg = heartbeat_config(context);
    let store = match heartbeat_store(context) {
        Some(s) => s,
        None => {
            return json!({
                "maintenance_status": "error",
                "action": "runner",
                "runner_status": "stopped",
                "runner_exit_reason": "store_error",
                "runner_iterations": 0,
                "error": "failed to open maintenance heartbeat store",
            });
        }
    };

    let mut runner = match MaintenanceHeartbeatRunner::new(
        context.paths.project_id(),
        runner_id,
        Some(std::process::id()),
        "running",
        Some("manual".to_string()),
        Some(now.to_string()),
        Some(now.to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
    ) {
        Ok(r) => r,
        Err(err) => {
            return json!({
                "maintenance_status": "error",
                "action": "runner",
                "runner_status": "stopped",
                "runner_exit_reason": "constructor_error",
                "runner_iterations": 0,
                "error": err.to_string(),
            });
        }
    };
    let _ = store.save_runner(&runner);

    let mut iterations: u32 = 0;
    let exit_reason: String;

    loop {
        let observed_at = utc_now_iso();

        if !cfg.enabled {
            exit_reason = "disabled".to_string();
            break;
        }
        if !assessor_present(context, &cfg.assessor) {
            exit_reason = format!("assessor_missing:{}", cfg.assessor);
            break;
        }

        let schedule = store.load_schedule();
        if schedule_is_future(&schedule, &observed_at) {
            let next_run_at = schedule.value.as_ref().and_then(|s| s.next_run_at.clone());
            runner.state = "sleeping".to_string();
            runner.last_seen_at = Some(observed_at.clone());
            runner.observed_next_run_at = next_run_at.clone();
            runner.sleep_until = next_run_at.clone();
            runner.exit_reason = None;
            let _ = store.save_runner(&runner);

            iterations += 1;
            if iterations >= max_iterations {
                exit_reason = "max_iterations".to_string();
                break;
            }

            let delay = seconds_until(&observed_at, next_run_at.as_deref()).min(sleep_cap_s as f64);
            if delay > 0.0 {
                std::thread::sleep(std::time::Duration::from_secs_f64(delay));
            }
            continue;
        }

        runner.state = "running".to_string();
        runner.last_seen_at = Some(observed_at.clone());
        runner.last_wake_at = Some(observed_at.clone());
        runner.observed_next_run_at = schedule.value.as_ref().and_then(|s| s.next_run_at.clone());
        runner.sleep_until = None;
        runner.exit_reason = None;
        let _ = store.save_runner(&runner);

        let tick_payload = maintenance_tick(context, client, false, no_dispatch, &observed_at);
        let tick_status = tick_payload
            .get("tick_status")
            .and_then(|v| v.as_str())
            .map(String::from);
        let observed_next_run_at = tick_payload
            .get("schedule")
            .and_then(|v| v.get("record"))
            .and_then(|v| v.get("next_run_at"))
            .and_then(|v| v.as_str())
            .map(String::from);

        runner.last_seen_at = Some(utc_now_iso());
        runner.last_tick_at = Some(observed_at.clone());
        runner.last_tick_status = tick_status;
        runner.observed_next_run_at = observed_next_run_at;
        runner.sleep_until = None;
        runner.exit_reason = None;
        let _ = store.save_runner(&runner);

        iterations += 1;
        if iterations >= max_iterations {
            exit_reason = "max_iterations".to_string();
            break;
        }
    }

    runner.state = "stopped".to_string();
    runner.last_seen_at = Some(utc_now_iso());
    runner.sleep_until = None;
    runner.exit_reason = Some(exit_reason.clone());
    let _ = store.save_runner(&runner);

    json!({
        "maintenance_status": "ok",
        "action": "runner",
        "runner_status": "stopped",
        "runner_id": runner_id,
        "runner_pid": std::process::id(),
        "runner_exit_reason": exit_reason,
        "runner_iterations": iterations,
    })
}

fn schedule_is_future(
    schedule: &MaintenanceHeartbeatReadResult<MaintenanceHeartbeatSchedule>,
    now: &str,
) -> bool {
    if let Some(value) = &schedule.value {
        if let Some(next_run_at) = &value.next_run_at {
            if let Some(secs) = ccb_heartbeat::seconds_between(now, next_run_at) {
                return secs > 0.0;
            }
        }
    }
    false
}

fn seconds_until(from: &str, until: Option<&str>) -> f64 {
    if let Some(until) = until {
        if let Some(secs) = ccb_heartbeat::seconds_between(from, until) {
            return secs.max(0.0);
        }
    }
    0.0
}

fn next_unknown_streak(
    health: &str,
    previous: &MaintenanceHeartbeatReadResult<MaintenanceHeartbeatStatus>,
) -> u32 {
    if health != HEALTH_UNKNOWN {
        return 0;
    }
    previous
        .value
        .as_ref()
        .map(|s| s.unknown_streak)
        .unwrap_or(0)
        + 1
}

fn next_after_s(health: &str, cfg: &HeartbeatConfig, unknown_streak: u32) -> u32 {
    if health == HEALTH_HEALTHY {
        return cfg.interval_s;
    }
    if health == HEALTH_UNKNOWN && unknown_streak >= cfg.unknown_streak_cap {
        return cfg.interval_s;
    }
    cfg.min_interval_s
}

fn first_issue_reason(evidence: &[Value]) -> Option<String> {
    evidence.first().and_then(|item| {
        item.get("reason")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| item.get("kind").and_then(|v| v.as_str()).map(String::from))
    })
}

fn activation_needs_user(activation: Option<&MaintenanceHeartbeatActivation>) -> bool {
    activation
        .map(|a| a.status == "blocked" || a.status == "failed")
        .unwrap_or(false)
}

struct ActivationCommon {
    project_id: String,
    activation_id: String,
    condition_kind: String,
    trigger_kind: String,
    source: String,
    observed_at: String,
    target_agent: String,
    delivery_mode: String,
    payload_kind: String,
    dedup_key: String,
    reason: String,
    created_by: String,
}

#[allow(clippy::too_many_arguments)]
fn build_activation(
    common: &ActivationCommon,
    status: &str,
    job_id: Option<String>,
    submitted_at: Option<String>,
    suppressed_reason: Option<String>,
    error: Option<String>,
    payload_summary: Option<Value>,
    evidence: Vec<Value>,
) -> Result<MaintenanceHeartbeatActivation, ccb_heartbeat::HeartbeatError> {
    MaintenanceHeartbeatActivation::new(
        common.project_id.clone(),
        common.activation_id.clone(),
        status,
        common.condition_kind.clone(),
        common.trigger_kind.clone(),
        common.source.clone(),
        common.observed_at.clone(),
        common.target_agent.clone(),
        common.delivery_mode.clone(),
        common.payload_kind.clone(),
        common.dedup_key.clone(),
        common.reason.clone(),
        common.created_by.clone(),
        None,
        None,
        job_id,
        submitted_at,
        suppressed_reason,
        error,
        1,
        payload_summary,
        evidence,
    )
}

#[allow(clippy::too_many_arguments)]
fn maybe_activate(
    context: &CliContext,
    cfg: &HeartbeatConfig,
    store: &MaintenanceHeartbeatStore,
    evaluation: &MaintenanceHeartbeatEvaluation,
    now: &str,
    next_after_s: u32,
    no_dispatch: bool,
    client: &dyn crate::services::DaemonClient,
) -> Result<Option<MaintenanceHeartbeatActivation>, Value> {
    if evaluation.health == HEALTH_HEALTHY {
        return Ok(None);
    }
    if cfg.assessor.is_empty() {
        return Ok(None);
    }

    let target = cfg.assessor.clone();
    let activation_id = activation_id();
    let source = evaluation.source_kind.clone();
    let reason =
        first_issue_reason(&evaluation.evidence).unwrap_or_else(|| evaluation.health.clone());
    let dedup_key = diagnostic_dedup_key(context, evaluation);

    let common = ActivationCommon {
        project_id: context.paths.project_id().to_string(),
        activation_id: activation_id.clone(),
        condition_kind: "heartbeat_state_check".to_string(),
        trigger_kind: "state_check".to_string(),
        source: source.clone(),
        observed_at: now.to_string(),
        target_agent: target.clone(),
        delivery_mode: "ask_silence".to_string(),
        payload_kind: "maintenance_diagnostic".to_string(),
        dedup_key: dedup_key.clone(),
        reason: reason.clone(),
        created_by: "maintenance_tick".to_string(),
    };

    let summary = Some(activation_summary(evaluation, next_after_s));
    let evidence: Vec<Value> = evaluation.evidence.iter().take(5).cloned().collect();

    let activation = if !assessor_present(context, &target) {
        build_activation(
            &common,
            "blocked",
            None,
            None,
            Some("assessor_missing".to_string()),
            Some(format!("configured assessor is not present: {target}")),
            summary.clone(),
            evidence.clone(),
        )
    } else if no_dispatch {
        build_activation(
            &common,
            "suppressed",
            None,
            None,
            Some("dispatch_disabled".to_string()),
            None,
            summary.clone(),
            evidence.clone(),
        )
    } else {
        match dispatch_activation(
            context,
            client,
            &target,
            &activation_id,
            &dedup_key,
            now,
            evaluation,
            next_after_s,
        ) {
            Ok(job_id) => build_activation(
                &common,
                "submitted",
                job_id,
                Some(now.to_string()),
                None,
                None,
                summary.clone(),
                evidence.clone(),
            ),
            Err(err) => build_activation(
                &common,
                "failed",
                None,
                None,
                None,
                Some(err),
                summary.clone(),
                evidence.clone(),
            ),
        }
    };

    let activation = match activation {
        Ok(a) => a,
        Err(err) => {
            return Err(json!({
                "maintenance_status": "error",
                "action": "tick",
                "tick_status": "error",
                "tick_source_kind": evaluation.source_kind.clone(),
                "tick_recommended_action": evaluation.recommended_action(),
                "tick_needs_user": false,
                "tick_next_heartbeat_after_s": next_after_s,
                "status_written": false,
                "schedule_written": false,
                "activation_written": false,
                "tick_activation_status": Value::Null,
                "tick_activation_id": Value::Null,
                "tick_activation_job_id": Value::Null,
                "tick_summary": evaluation.summary.clone(),
                "tick_evidence": evaluation.evidence.clone(),
                "error": err.to_string(),
            }));
        }
    };

    let _ = store.append_activation(&activation);
    Ok(Some(activation))
}

#[allow(clippy::too_many_arguments)]
fn dispatch_activation(
    context: &CliContext,
    client: &dyn crate::services::DaemonClient,
    target_agent: &str,
    activation_id: &str,
    dedup_key: &str,
    observed_at: &str,
    evaluation: &MaintenanceHeartbeatEvaluation,
    next_after_s: u32,
) -> Result<Option<String>, String> {
    let diagnostic = json!({
        "schema_version": 1,
        "record_type": "maintenance_heartbeat_diagnostic",
        "activation_id": activation_id,
        "project_id": context.paths.project_id(),
        "project": context.project.project_root.to_string_lossy().to_string(),
        "observed_at": observed_at,
        "health": evaluation.health,
        "source_kind": evaluation.source_kind,
        "dedup_key": dedup_key,
        "recommended_action": evaluation.recommended_action(),
        "next_heartbeat_after_s": next_after_s,
        "summary": evaluation.summary,
        "evidence": evaluation.evidence.iter().take(5).collect::<Vec<_>>(),
    });
    let body = format!(
        "CCB maintenance heartbeat detected a runtime condition that needs semantic supervision.\n\nDiagnostic package:\n```json\n{}\n```",
        serde_json::to_string_pretty(&diagnostic).unwrap_or_else(|_| "{}".to_string())
    );

    let result = client.call(
        "submit",
        json!({
            "project_id": context.paths.project_id(),
            "to_agent": target_agent,
            "from_actor": "maintenance-heartbeat",
            "body": body,
            "task_id": format!("maintenance-heartbeat:{dedup_key}"),
            "message_type": "ask",
            "delivery_scope": "single",
            "silence_on_success": true,
            "route_options": json!({
                "maintenance_heartbeat": true,
                "activation_id": activation_id,
                "dedup_key": dedup_key,
            }),
        }),
    )?;

    Ok(submitted_job_id(&result))
}

fn submitted_job_id(payload: &Value) -> Option<String> {
    payload
        .get("job_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("jobs")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("job_id"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .filter(|s| !s.is_empty())
        })
}

fn activation_summary(evaluation: &MaintenanceHeartbeatEvaluation, next_after_s: u32) -> Value {
    let mut summary = serde_json::Map::new();
    summary.insert("health".into(), evaluation.health.clone().into());
    summary.insert("source_kind".into(), evaluation.source_kind.clone().into());
    summary.insert(
        "recommended_action".into(),
        evaluation.recommended_action().into(),
    );
    summary.insert("next_heartbeat_after_s".into(), next_after_s.into());
    if let Some(obj) = evaluation.summary.as_object() {
        for (key, value) in obj {
            if value.is_string() || value.is_number() || value.is_boolean() || value.is_null() {
                summary.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(summary)
}

fn activation_id() -> String {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("act_{:x}_{}", ns, std::process::id())
}

fn diagnostic_dedup_key(
    context: &CliContext,
    evaluation: &MaintenanceHeartbeatEvaluation,
) -> String {
    format!(
        "{}:{}:{}:{}",
        context.paths.project_id(),
        evaluation.health,
        evaluation.source_kind,
        evaluation.evidence.len()
    )
}
