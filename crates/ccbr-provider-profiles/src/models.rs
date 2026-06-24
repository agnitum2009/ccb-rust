use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const VALID_PROFILE_MODES: &[&str] = &["inherit", "overlay", "isolated"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfileSpec {
    #[serde(default = "default_mode")]
    pub mode: String,
    pub home: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub inherit_api: bool,
    #[serde(default = "default_true")]
    pub inherit_auth: bool,
    #[serde(default = "default_true")]
    pub inherit_config: bool,
    #[serde(default = "default_true")]
    pub inherit_skills: bool,
    #[serde(default = "default_true")]
    pub inherit_commands: bool,
    #[serde(default = "default_true")]
    pub inherit_memory: bool,
}

fn default_mode() -> String {
    "inherit".into()
}
fn default_true() -> bool {
    true
}

impl Default for ProviderProfileSpec {
    fn default() -> Self {
        Self {
            mode: "inherit".into(),
            home: None,
            env: HashMap::new(),
            inherit_api: true,
            inherit_auth: true,
            inherit_config: true,
            inherit_skills: true,
            inherit_commands: true,
            inherit_memory: true,
        }
    }
}

impl ProviderProfileSpec {
    /// Normalize and validate the spec.
    pub fn validate(&self) -> Result<(), crate::ProfilesError> {
        let mode = self.mode.trim().to_lowercase();
        if !VALID_PROFILE_MODES.contains(&mode.as_str()) {
            return Err(crate::ProfilesError::Validation(format!(
                "provider_profile.mode must be one of: {}",
                VALID_PROFILE_MODES.join(", ")
            )));
        }
        Ok(())
    }

    /// Return a normalized copy with trimmed strings and lowercase mode.
    pub fn normalized(&self) -> Self {
        let mode = self.mode.trim().to_lowercase();
        let home = self
            .home
            .as_ref()
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty());
        let env = self
            .env
            .iter()
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        Self {
            mode,
            home,
            env,
            inherit_api: self.inherit_api,
            inherit_auth: self.inherit_auth,
            inherit_config: self.inherit_config,
            inherit_skills: self.inherit_skills,
            inherit_commands: self.inherit_commands,
            inherit_memory: self.inherit_memory,
        }
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": self.mode,
            "home": self.home,
            "env": self.env,
            "inherit_api": self.inherit_api,
            "inherit_auth": self.inherit_auth,
            "inherit_config": self.inherit_config,
            "inherit_skills": self.inherit_skills,
            "inherit_commands": self.inherit_commands,
            "inherit_memory": self.inherit_memory,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedProviderProfile {
    pub provider: String,
    pub agent_name: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    pub profile_root: Option<String>,
    pub runtime_home: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub inherit_api: bool,
    #[serde(default = "default_true")]
    pub inherit_auth: bool,
    #[serde(default = "default_true")]
    pub inherit_config: bool,
    #[serde(default = "default_true")]
    pub inherit_skills: bool,
    #[serde(default = "default_true")]
    pub inherit_commands: bool,
    #[serde(default = "default_true")]
    pub inherit_memory: bool,
}

impl ResolvedProviderProfile {
    pub fn new(provider: impl Into<String>, agent_name: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            agent_name: agent_name.into(),
            mode: "inherit".into(),
            profile_root: None,
            runtime_home: None,
            env: HashMap::new(),
            inherit_api: true,
            inherit_auth: true,
            inherit_config: true,
            inherit_skills: true,
            inherit_commands: true,
            inherit_memory: true,
        }
    }

    pub fn validate(&self) -> Result<(), crate::ProfilesError> {
        let provider = self.provider.trim().to_lowercase();
        if provider.is_empty() {
            return Err(crate::ProfilesError::Validation(
                "provider cannot be empty".into(),
            ));
        }
        let agent_name = self.agent_name.trim().to_lowercase();
        if agent_name.is_empty() {
            return Err(crate::ProfilesError::Validation(
                "agent_name cannot be empty".into(),
            ));
        }
        let mode = self.mode.trim().to_lowercase();
        if !VALID_PROFILE_MODES.contains(&mode.as_str()) {
            return Err(crate::ProfilesError::Validation(format!(
                "mode must be one of: {}",
                VALID_PROFILE_MODES.join(", ")
            )));
        }
        Ok(())
    }

    pub fn normalized(&self) -> Self {
        let provider = self.provider.trim().to_lowercase();
        let agent_name = self.agent_name.trim().to_lowercase();
        let mode = self.mode.trim().to_lowercase();
        let profile_root = normalize_path_text(self.profile_root.as_deref());
        let runtime_home = normalize_path_text(self.runtime_home.as_deref());
        let env = self
            .env
            .iter()
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        Self {
            provider,
            agent_name,
            mode,
            profile_root,
            runtime_home,
            env,
            inherit_api: self.inherit_api,
            inherit_auth: self.inherit_auth,
            inherit_config: self.inherit_config,
            inherit_skills: self.inherit_skills,
            inherit_commands: self.inherit_commands,
            inherit_memory: self.inherit_memory,
        }
    }

    pub fn profile_root_path(&self) -> Option<PathBuf> {
        self.profile_root.as_ref().map(PathBuf::from)
    }

    pub fn runtime_home_path(&self) -> Option<PathBuf> {
        self.runtime_home.as_ref().map(PathBuf::from)
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "provider": self.provider,
            "agent_name": self.agent_name,
            "mode": self.mode,
            "profile_root": self.profile_root,
            "runtime_home": self.runtime_home,
            "env": self.env,
            "inherit_api": self.inherit_api,
            "inherit_auth": self.inherit_auth,
            "inherit_config": self.inherit_config,
            "inherit_skills": self.inherit_skills,
            "inherit_commands": self.inherit_commands,
            "inherit_memory": self.inherit_memory,
        })
    }

    pub fn from_record(
        record: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, crate::ProfilesError> {
        let provider = record
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let agent_name = record
            .get("agent_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mode = record
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("inherit")
            .to_string();
        let profile_root = record
            .get("profile_root")
            .and_then(|v| v.as_str())
            .map(String::from);
        let runtime_home = record
            .get("runtime_home")
            .and_then(|v| v.as_str())
            .map(String::from);
        let env = record
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let inherit_api = record
            .get("inherit_api")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let inherit_auth = record
            .get("inherit_auth")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let inherit_config = record
            .get("inherit_config")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let inherit_skills = record
            .get("inherit_skills")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let inherit_commands = record
            .get("inherit_commands")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let inherit_memory = record
            .get("inherit_memory")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let resolved = Self {
            provider,
            agent_name,
            mode,
            profile_root,
            runtime_home,
            env,
            inherit_api,
            inherit_auth,
            inherit_config,
            inherit_skills,
            inherit_commands,
            inherit_memory,
        };
        resolved.validate()?;
        Ok(resolved.normalized())
    }
}

pub fn normalize_path_text(value: Option<&str>) -> Option<String> {
    let raw = value.unwrap_or("").trim();
    if raw.is_empty() {
        return None;
    }
    let expanded = expand_tilde(raw);
    let path = Path::new(&expanded);
    let resolved = path.canonicalize().or_else(|_| std::path::absolute(path));
    match resolved {
        Ok(p) => Some(p.to_string_lossy().into_owned()),
        Err(_) => Some(path.to_string_lossy().into_owned()),
    }
}

fn expand_tilde(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_spec_default() {
        let spec = ProviderProfileSpec::default();
        assert_eq!(spec.mode, "inherit");
        assert!(spec.inherit_api);
    }

    #[test]
    fn test_profile_spec_validation_rejects_invalid_mode() {
        let spec = ProviderProfileSpec {
            mode: "custom".into(),
            ..Default::default()
        };
        assert!(spec.validate().is_err());
    }

    #[test]
    fn test_profile_spec_normalization_trims_home() {
        let spec = ProviderProfileSpec {
            home: Some("  /tmp/home  ".into()),
            env: [(" K ".into(), " V ".into())].into(),
            ..Default::default()
        };
        let normalized = spec.normalized();
        assert_eq!(normalized.home, Some("/tmp/home".into()));
        assert_eq!(normalized.env.get("K"), Some(&"V".into()));
    }

    #[test]
    fn test_resolved_profile_validation() {
        let profile = ResolvedProviderProfile::new("codex", "agent1");
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_resolved_profile_rejects_empty_provider() {
        let profile = ResolvedProviderProfile::new("", "agent1");
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_resolved_profile_round_trip_record() {
        let profile = ResolvedProviderProfile::new("codex", "agent1");
        let record = profile.to_record();
        let record_obj = record.as_object().unwrap().clone();
        let restored = ResolvedProviderProfile::from_record(&record_obj).unwrap();
        assert_eq!(profile, restored);
    }
}
