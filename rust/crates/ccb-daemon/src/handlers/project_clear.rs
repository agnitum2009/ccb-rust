use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_project_clear(_app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_names: Vec<String> = payload
        .get("agent_names")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    let all_names =
        if agent_names.is_empty() || agent_names.iter().any(|n| n.to_lowercase() == "all") {
            vec!["all".to_string()]
        } else {
            agent_names
        };
    Ok(json!({
        "status": "ok",
        "agent_names": all_names,
        "results": [],
    }))
}
