use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::models::api_models::messages::MessageEnvelope;

pub fn handle_submit(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let project_id = payload
        .get("project_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let to_agent = payload
        .get("to_agent")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let from_actor = payload
        .get("from_actor")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let body = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");

    if project_id.is_empty() {
        return Err("project_id required".into());
    }
    if to_agent.is_empty() {
        return Err("to_agent required".into());
    }
    if from_actor.is_empty() {
        return Err("from_actor required".into());
    }
    if body.is_empty() {
        return Err("body required".into());
    }

    let _message_type = payload
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("ask");
    let _delivery_scope = payload
        .get("delivery_scope")
        .and_then(|v| v.as_str())
        .unwrap_or("single");
    let _task_id = payload.get("task_id").and_then(|v| v.as_str());
    let _reply_to = payload.get("reply_to").and_then(|v| v.as_str());

    let envelope = MessageEnvelope {
        project_id: project_id.to_string(),
        to_agent: to_agent.to_string(),
        from_actor: from_actor.to_string(),
        body: body.to_string(),
        task_id: payload
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        reply_to: payload
            .get("reply_to")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        message_type: payload
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("ask")
            .to_string(),
        delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    };
    envelope.validate()?;

    let receipt = app.dispatcher.submit(&envelope);
    Ok(receipt.to_record())
}
