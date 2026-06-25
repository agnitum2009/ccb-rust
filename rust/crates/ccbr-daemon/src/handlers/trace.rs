use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_trace(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if target.is_empty() {
        return Err("trace requires target".into());
    }
    app.mailbox_control.try_trace(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn app() -> (TempDir, CcbdApp) {
        let dir = TempDir::new().unwrap();
        let app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        (dir, app)
    }

    #[test]
    fn trace_rejects_legacy_all_target_like_python_handler() {
        let (_dir, mut app) = app();

        let err = handle_trace(&mut app, &json!({"target": "all"})).unwrap_err();

        assert!(
            err.contains("trace requires <submission_id|message_id|attempt_id|reply_id|job_id>")
        );
    }

    #[test]
    fn trace_missing_job_returns_error_instead_of_panicking() {
        let (_dir, mut app) = app();

        let err = handle_trace(&mut app, &json!({"target": "job_missing"})).unwrap_err();

        assert!(err.contains("job not found in message bureau"));
    }
}
