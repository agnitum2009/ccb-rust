//! Test-only fault injection service migrated from `lib/fault_injection/`.
//!
//! This is intentionally minimal: it provides rule storage, arming, clearing,
//! and consumption so that daemon tests and future provider integration tests
//! can simulate failures without invoking real providers.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const VALID_FAILURE_REASONS: &[&str] = &["api_error", "transport_error"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultRule {
    pub rule_id: String,
    pub agent_name: String,
    pub task_id: String,
    pub reason: String,
    pub remaining_count: u32,
    pub error_message: String,
    pub created_at: String,
    pub updated_at: String,
}

impl FaultRule {
    pub fn validate(&self) -> Result<(), String> {
        if self.rule_id.trim().is_empty() {
            return Err("rule_id cannot be empty".into());
        }
        if self.task_id.trim().is_empty() {
            return Err("task_id cannot be empty".into());
        }
        if self.remaining_count == 0 {
            return Err("remaining_count must be positive".into());
        }
        let reason = self.reason.trim().to_lowercase();
        if !VALID_FAILURE_REASONS.contains(&reason.as_str()) {
            return Err(format!("unsupported fault reason: {}", self.reason));
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "rule_id": self.rule_id,
            "agent_name": self.agent_name,
            "task_id": self.task_id,
            "reason": self.reason,
            "remaining_count": self.remaining_count,
            "error_message": self.error_message,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumedFault {
    pub rule_id: String,
    pub agent_name: String,
    pub task_id: String,
    pub reason: String,
    pub error_message: String,
    pub remaining_count: u32,
    pub injected_at: String,
}

#[derive(Debug, Default)]
pub struct FaultInjectionService {
    rules: HashMap<String, FaultRule>,
}

impl FaultInjectionService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_rules(&self) -> Vec<&FaultRule> {
        let mut rules: Vec<&FaultRule> = self.rules.values().collect();
        rules.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.rule_id.cmp(&b.rule_id))
        });
        rules
    }

    pub fn arm_rule(
        &mut self,
        agent_name: &str,
        task_id: &str,
        reason: &str,
        count: u32,
        error_message: Option<&str>,
    ) -> Result<FaultRule, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let rule_id = format!(
            "flt_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        let rule = FaultRule {
            rule_id,
            agent_name: agent_name.to_string(),
            task_id: task_id.to_string(),
            reason: reason.to_lowercase(),
            remaining_count: count,
            error_message: error_message.unwrap_or("fault injection drill").to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        rule.validate()?;
        self.rules.insert(rule.rule_id.clone(), rule.clone());
        Ok(rule)
    }

    pub fn clear_rule(&mut self, target: &str) -> Result<Vec<FaultRule>, String> {
        let key = target.trim();
        if key.is_empty() {
            return Err("fault clear requires <rule_id|all>".into());
        }
        if key == "all" {
            let cleared: Vec<FaultRule> = self.rules.drain().map(|(_, v)| v).collect();
            return Ok(cleared);
        }
        match self.rules.remove(key) {
            Some(rule) => Ok(vec![rule]),
            None => Err(format!("fault rule not found: {key}")),
        }
    }

    pub fn consume(&mut self, agent_name: &str, task_id: &str) -> Option<ConsumedFault> {
        let task_id = task_id.trim();
        if task_id.is_empty() {
            return None;
        }
        let now = chrono::Utc::now().to_rfc3339();

        // Find the first matching rule id without mutating the map.
        let matched_id = self
            .rules
            .values()
            .find(|rule| rule.agent_name == agent_name && rule.task_id == task_id)
            .map(|rule| rule.rule_id.clone())?;

        let rule = self.rules.get_mut(&matched_id).expect("rule exists");
        let remaining_count = rule.remaining_count.saturating_sub(1);
        let consumed = ConsumedFault {
            rule_id: rule.rule_id.clone(),
            agent_name: rule.agent_name.clone(),
            task_id: rule.task_id.clone(),
            reason: rule.reason.clone(),
            error_message: rule.error_message.clone(),
            remaining_count,
            injected_at: now.clone(),
        };

        if remaining_count > 0 {
            rule.remaining_count = remaining_count;
            rule.updated_at = now;
        } else {
            self.rules.remove(&matched_id);
        }

        Some(consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm_and_consume_fault() {
        let mut svc = FaultInjectionService::new();
        let rule = svc
            .arm_rule("claude", "task-1", "api_error", 2, None)
            .unwrap();
        assert_eq!(rule.remaining_count, 2);

        let consumed = svc.consume("claude", "task-1").unwrap();
        assert_eq!(consumed.remaining_count, 1);

        let _ = svc.consume("claude", "task-1").unwrap();
        assert!(svc.consume("claude", "task-1").is_none());
    }

    #[test]
    fn test_clear_all_rules() {
        let mut svc = FaultInjectionService::new();
        svc.arm_rule("claude", "task-1", "api_error", 1, None)
            .unwrap();
        svc.arm_rule("gemini", "task-2", "transport_error", 1, None)
            .unwrap();
        assert_eq!(svc.clear_rule("all").unwrap().len(), 2);
        assert!(svc.list_rules().is_empty());
    }
}
