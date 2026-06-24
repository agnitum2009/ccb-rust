use std::collections::HashMap;

use ccbr_provider_core::runtime_specs::parse_qualified_provider;
use ccbr_terminal::backend::TerminalBackend;
use ccbr_terminal::registry::UserSession;
use serde_json::{Map, Value};

/// Extract the provider map from registry data.
pub fn get_providers_map(data: &Map<String, Value>) -> HashMap<String, Map<String, Value>> {
    data.get("providers")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_object().map(|o| (k.trim().to_lowercase(), o.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// Check whether the pane for `provider` is alive in `record`.
pub fn provider_pane_alive<F>(
    record: &Map<String, Value>,
    provider: &str,
    get_backend_for_session_fn: &F,
) -> bool
where
    F: Fn(&UserSession) -> Option<Box<dyn TerminalBackend>>,
{
    let (base_provider, _) = parse_qualified_provider(provider);
    let providers = get_providers_map(record);
    let normalized = provider.trim().to_lowercase();
    let entry = providers
        .get(&normalized)
        .or_else(|| providers.get(&base_provider));
    let entry = match entry {
        Some(e) => e,
        None => return false,
    };

    let pane_id = entry
        .get("pane_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .unwrap_or("");
    if pane_id.is_empty() {
        return false;
    }

    let terminal = record
        .get("terminal")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "tmux".to_string());

    let backend = match get_backend_for_session_fn(&UserSession {
        terminal: Some(terminal),
        ..Default::default()
    }) {
        Some(b) => b,
        None => return false,
    };

    backend.is_alive(pane_id).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockBackend {
        alive: HashMap<String, bool>,
    }

    impl TerminalBackend for MockBackend {
        fn send_text(&self, _pane_id: &str, _text: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn is_alive(&self, pane_id: &str) -> ccbr_terminal::backend::Result<bool> {
            Ok(self.alive.get(pane_id).copied().unwrap_or(false))
        }
        fn kill_pane(&self, _pane_id: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn activate(&self, _pane_id: &str) -> ccbr_terminal::backend::Result<()> {
            Ok(())
        }
        fn create_pane(
            &self,
            _cmd: &str,
            _cwd: &str,
            _direction: &str,
            _percent: u32,
            _parent_pane: Option<&str>,
        ) -> ccbr_terminal::backend::Result<String> {
            Ok("%0".into())
        }
    }

    fn session_fn(_session: &UserSession) -> Option<Box<dyn TerminalBackend>> {
        let mut alive = HashMap::new();
        alive.insert("%1".into(), true);
        alive.insert("%2".into(), false);
        Some(Box::new(MockBackend { alive }))
    }

    #[test]
    fn test_get_providers_map_normalizes_keys() {
        let mut data = Map::new();
        let mut providers = Map::new();
        let mut claude = Map::new();
        claude.insert("pane_id".into(), "%1".into());
        providers.insert("Claude".into(), Value::Object(claude));
        data.insert("providers".into(), Value::Object(providers));

        let map = get_providers_map(&data);
        assert!(map.contains_key("claude"));
    }

    #[test]
    fn test_provider_pane_alive_alive() {
        let mut data = Map::new();
        let mut providers = Map::new();
        let mut claude = Map::new();
        claude.insert("pane_id".into(), "%1".into());
        providers.insert("claude".into(), Value::Object(claude));
        data.insert("providers".into(), Value::Object(providers));

        assert!(provider_pane_alive(&data, "claude", &session_fn));
    }

    #[test]
    fn test_provider_pane_alive_dead() {
        let mut data = Map::new();
        let mut providers = Map::new();
        let mut claude = Map::new();
        claude.insert("pane_id".into(), "%2".into());
        providers.insert("claude".into(), Value::Object(claude));
        data.insert("providers".into(), Value::Object(providers));

        assert!(!provider_pane_alive(&data, "claude", &session_fn));
    }

    #[test]
    fn test_provider_pane_alive_missing_provider() {
        let data = Map::new();
        assert!(!provider_pane_alive(&data, "claude", &session_fn));
    }

    #[test]
    fn test_provider_pane_alive_missing_pane_id() {
        let mut data = Map::new();
        let mut providers = Map::new();
        providers.insert("claude".into(), Value::Object(Map::new()));
        data.insert("providers".into(), Value::Object(providers));

        assert!(!provider_pane_alive(&data, "claude", &session_fn));
    }
}
