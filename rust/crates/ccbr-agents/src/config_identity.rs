use crate::models::ProjectConfig;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct ConfigIdentityPayload {
    pub known_agents: Vec<String>,
    pub config_signature: String,
}

/// Compute the identity payload for a project config.
///
/// This removes volatile fields (schema_version, record_type, source_path, sidebar_view)
/// and computes a SHA256 hash of the normalized config for change detection.
pub fn project_config_identity_payload(config: &ProjectConfig) -> ConfigIdentityPayload {
    let mut canonical = config.to_record();

    // Remove volatile fields that shouldn't affect identity
    if let Some(obj) = canonical.as_object_mut() {
        obj.remove("schema_version");
        obj.remove("record_type");
        obj.remove("source_path");
        obj.remove("sidebar_view");
    }

    // Clean up tool_windows - remove label and show_in_sidebar
    if let Some(tools) = canonical
        .get_mut("tool_windows")
        .and_then(|v| v.as_array_mut())
    {
        for tool in tools.iter_mut().filter_map(|t| t.as_object_mut()) {
            tool.remove("label");
            tool.remove("show_in_sidebar");
        }
    }

    // Clean up agent specs - remove schema_version and record_type
    if let Some(agents) = canonical.get_mut("agents").and_then(|v| v.as_object_mut()) {
        for agent_spec in agents.values_mut().filter_map(|v| v.as_object_mut()) {
            agent_spec.remove("schema_version");
            agent_spec.remove("record_type");
        }
    }

    // Encode with sorted keys and compact separators
    let encoded = serde_json::to_string(&canonical).unwrap_or_default();

    // Compute SHA256 hash
    let hash = Sha256::digest(encoded.as_bytes());
    let config_signature = hex::encode(hash);

    // Get known agents from config
    let mut known_agents: Vec<String> = config.agents.keys().cloned().collect();
    known_agents.sort();

    ConfigIdentityPayload {
        known_agents,
        config_signature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentSpec, WorkspaceMode};
    use std::collections::HashMap;

    #[test]
    fn test_config_identity_payload() {
        let mut agents = HashMap::new();
        agents.insert(
            "agent1".into(),
            AgentSpec {
                name: "agent1".into(),
                provider: "codex".into(),
                target: ".".into(),
                workspace_mode: WorkspaceMode::Inplace,
                ..AgentSpec::default_with_name("agent1")
            },
        );

        let config = ProjectConfig {
            version: 2,
            default_agents: vec!["agent1".into()],
            agents,
            ..ProjectConfig::default()
        };

        let payload = project_config_identity_payload(&config);

        assert!(!payload.known_agents.is_empty());
        assert!(!payload.config_signature.is_empty());
        assert_eq!(payload.known_agents, vec!["agent1"]);
    }

    #[test]
    fn test_config_identity_signature_stable() {
        let mut agents = HashMap::new();
        agents.insert(
            "test".into(),
            AgentSpec {
                name: "test".into(),
                provider: "codex".into(),
                target: ".".into(),
                workspace_mode: WorkspaceMode::Inplace,
                ..AgentSpec::default_with_name("test")
            },
        );

        let config1 = ProjectConfig {
            version: 2,
            default_agents: vec!["test".into()],
            agents: agents.clone(),
            ..ProjectConfig::default()
        };

        let config2 = ProjectConfig {
            version: 2,
            default_agents: vec!["test".into()],
            agents,
            ..ProjectConfig::default()
        };

        let payload1 = project_config_identity_payload(&config1);
        let payload2 = project_config_identity_payload(&config2);

        assert_eq!(payload1.config_signature, payload2.config_signature);
    }
}
