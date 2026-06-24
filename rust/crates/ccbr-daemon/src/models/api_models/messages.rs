use super::common::DeliveryScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub project_id: String,
    pub to_agent: String,
    pub from_actor: String,
    pub body: String,
    pub task_id: Option<String>,
    pub reply_to: Option<String>,
    pub message_type: String,
    pub delivery_scope: DeliveryScope,
    #[serde(default)]
    pub silence_on_success: bool,
    #[serde(default)]
    pub route_options: serde_json::Value,
    #[serde(default)]
    pub body_artifact: Option<serde_json::Value>,
}

impl MessageEnvelope {
    /// Normalize agent/actor identifiers in place.
    pub fn normalize(&mut self) -> Result<(), String> {
        self.to_agent = super::common::normalize_agent_name(&self.to_agent);
        self.from_actor = super::common::normalize_actor_name(&self.from_actor)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.project_id.is_empty() {
            return Err("project_id cannot be empty".into());
        }
        if self.to_agent.is_empty() {
            return Err("to_agent cannot be empty".into());
        }
        if self.from_actor.is_empty() {
            return Err("from_actor cannot be empty".into());
        }
        if self.body.trim().is_empty() {
            return Err("body cannot be empty".into());
        }
        if self.to_agent == "all"
            && matches!(self.delivery_scope, super::common::DeliveryScope::Single)
        {
            return Err("delivery_scope 'single' cannot target 'all'".into());
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "project_id": self.project_id,
            "to_agent": self.to_agent,
            "from_actor": self.from_actor,
            "body": self.body,
            "task_id": self.task_id,
            "reply_to": self.reply_to,
            "message_type": self.message_type,
            "delivery_scope": self.delivery_scope,
            "silence_on_success": self.silence_on_success,
            "route_options": self.route_options,
            "body_artifact": self.body_artifact,
        })
    }
}
