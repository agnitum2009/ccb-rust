//! Mirrors Python `lib/ccbd/reload_apply_graph.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_plan::project_config_identity_payload;

/// Build the target service graph for a reload apply.
pub fn build_reload_service_graph(
    _app: &CcbdApp,
    new_config: &ccbr_agents::models::ProjectConfig,
) -> ServiceGraph {
    let identity = project_config_identity_payload(new_config);
    let signature = identity
        .get("config_signature")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();
    ServiceGraph {
        version: Some(uuid::Uuid::new_v4().to_string()),
        config: new_config.clone(),
        config_identity: identity,
        config_signature: signature,
    }
}
