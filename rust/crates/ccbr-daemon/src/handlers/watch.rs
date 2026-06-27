use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_watch(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .trim();
    if target.is_empty() {
        return Err("watch requires target".into());
    }
    let start_line = payload
        .get("cursor")
        .or_else(|| payload.get("start_line"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if start_line < 0 {
        return Err("watch cursor cannot be negative".into());
    }
    let start_line = start_line as u64;
    Ok(app.dispatcher.watch(target, start_line))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api_models::messages::MessageEnvelope;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn app_with_job() -> CcbdApp {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let _ = app.dispatcher.submit(
            &MessageEnvelope {
                project_id: "proj-1".into(),
                to_agent: "agent1".into(),
                from_actor: "user".into(),
                body: "hello".into(),
                task_id: None,
                reply_to: None,
                message_type: "ask".into(),
                delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
                silence_on_success: false,
                route_options: json!({}),
                body_artifact: None,
            },
            "codex",
            None,
        );
        app
    }

    #[test]
    fn watch_uses_python_cursor_payload() {
        let mut app = app_with_job();

        let result = handle_watch(&mut app, &json!({"target": "agent1", "cursor": 1})).unwrap();

        assert_eq!(result["cursor"], 1);
        assert!(result["lines"].as_array().unwrap().is_empty());
    }

    #[test]
    fn watch_rejects_negative_cursor() {
        let mut app = app_with_job();

        let err = handle_watch(&mut app, &json!({"target": "agent1", "cursor": -1})).unwrap_err();

        assert!(err.contains("cursor cannot be negative"));
    }
}
