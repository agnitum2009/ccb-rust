use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::fault_injection::VALID_FAILURE_REASONS;

pub fn handle_fault_list(app: &mut CcbdApp, _payload: &Value) -> Result<Value, String> {
    let rules: Vec<Value> = app
        .fault_service
        .list_rules()
        .into_iter()
        .map(|r| r.to_record())
        .collect();
    Ok(json!({
        "project_id": app.project_id(),
        "rule_count": rules.len(),
        "rules": rules,
    }))
}

pub fn handle_fault_arm(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let task_id = payload
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Err("fault arm requires <agent_name>".into());
    }
    if task_id.is_empty() {
        return Err("fault arm requires --task-id".into());
    }
    if app.registry.get(agent_name).is_none() {
        return Err(format!("unknown agent: {agent_name}"));
    }
    let reason = payload
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("api_error")
        .trim()
        .to_lowercase();
    if !VALID_FAILURE_REASONS.contains(&reason.as_str()) {
        return Err(format!("unsupported fault reason: {reason}"));
    }
    let count = payload
        .get("count")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(1);
    if count == 0 {
        return Err("fault arm count must be positive".into());
    }
    let error_message = payload
        .get("error_message")
        .and_then(|v| v.as_str())
        .unwrap_or("fault injection drill");

    let rule =
        app.fault_service
            .arm_rule(agent_name, task_id, &reason, count, Some(error_message))?;
    Ok(json!({
        "project_id": app.project_id(),
        "rule_id": rule.rule_id,
        "agent_name": rule.agent_name,
        "task_id": rule.task_id,
        "reason": rule.reason,
        "remaining_count": rule.remaining_count,
        "error_message": rule.error_message,
    }))
}

pub fn handle_fault_clear(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if target.is_empty() {
        return Err("fault clear requires <rule_id|all>".into());
    }
    let cleared = app.fault_service.clear_rule(target)?;
    let rule_ids: Vec<&str> = cleared.iter().map(|r| r.rule_id.as_str()).collect();
    Ok(json!({
        "project_id": app.project_id(),
        "target": target,
        "cleared_count": cleared.len(),
        "cleared_rule_ids": rule_ids,
    }))
}
