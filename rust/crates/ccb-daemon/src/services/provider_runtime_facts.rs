//! Mirrors Python `lib/ccbd/services/provider_runtime_facts.py`.

use std::path::Path;

use crate::services::health_assessment::models::SessionBinding;
use ccb_provider_core::instance_resolution::named_agent_instance;
use ccb_provider_core::session_binding::Session;

/// Load a provider session for an agent using the supplied binding.
///
/// Mirrors Python `load_provider_session`.
pub fn load_provider_session(
    binding: &dyn SessionBinding,
    workspace_path: &Path,
    agent_name: &str,
) -> Option<Session> {
    let instance = named_agent_instance(agent_name, binding.provider());
    binding.load_session(workspace_path, instance.as_deref())
}
