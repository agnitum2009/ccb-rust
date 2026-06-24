use std::collections::HashMap;

use crate::session_binding::{Session, SessionBackend};

/// Result of inspecting tmux pane ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxPaneOwnership {
    pub state: String,
    pub pane_id: Option<String>,
    pub pane_title: Option<String>,
    pub expected_options: Vec<(String, String)>,
    pub actual_options: Vec<(String, String)>,
    pub reason: Option<String>,
}

impl TmuxPaneOwnership {
    /// Return true when the pane is considered owned by CCBR.
    pub fn is_owned(&self) -> bool {
        self.state == "owned"
    }
}

/// Apply tmux identity (pane title and user options) for a session.
pub fn apply_session_tmux_identity(session: &Session, backend: &dyn SessionBackend, pane_id: &str) {
    let pane_text = pane_id.trim();
    if pane_text.is_empty() {
        return;
    }
    if let Some(title) = session_display_title(session) {
        let _ = backend.set_pane_title(pane_text, &title);
    }
    for (name, value) in session_user_option_lookup(session) {
        let _ = backend.set_pane_user_option(pane_text, &name, &value);
    }
}

/// Inspect whether a tmux pane is owned by CCBR.
pub fn inspect_tmux_pane_ownership(
    session: &Session,
    backend: &dyn SessionBackend,
    pane_id: &str,
) -> TmuxPaneOwnership {
    let pane_text = pane_id.trim();
    if pane_text.is_empty() {
        return TmuxPaneOwnership {
            state: "unknown".to_string(),
            pane_id: None,
            pane_title: None,
            expected_options: Vec::new(),
            actual_options: Vec::new(),
            reason: Some("pane-id-missing".to_string()),
        };
    }

    let expected_items = expected_option_items(session);
    if expected_items.is_empty() {
        return TmuxPaneOwnership {
            state: "owned".to_string(),
            pane_id: Some(pane_text.to_string()),
            pane_title: None,
            expected_options: Vec::new(),
            actual_options: Vec::new(),
            reason: Some("ownership-not-recorded".to_string()),
        };
    }

    if let Some(ownership) = inspect_described_pane(backend, pane_text, &expected_items) {
        return ownership;
    }

    if let Some(ownership) = inspect_listed_panes(backend, pane_text, &expected_items) {
        return ownership;
    }

    TmuxPaneOwnership {
        state: "owned".to_string(),
        pane_id: Some(pane_text.to_string()),
        pane_title: None,
        expected_options: expected_items.clone(),
        actual_options: Vec::new(),
        reason: Some("inspection-unavailable".to_string()),
    }
}

/// Build user option lookup for a session.
pub fn session_user_option_lookup(session: &Session) -> HashMap<String, String> {
    if let Some(resolved) = session.user_option_lookup.as_ref() {
        let normalized = normalize_option_map(resolved);
        if !normalized.is_empty() {
            return normalized;
        }
    }
    let mut lookup = HashMap::new();
    if let Some(agent_name) = session_data_text(session, "agent_name") {
        lookup.insert("@ccbr_agent".to_string(), agent_name);
    }
    if let Some(project_id) = session_data_text(session, "ccbr_project_id") {
        lookup.insert("@ccbr_project_id".to_string(), project_id);
    }
    if let Some(session_id) = session_data_text(session, "ccbr_session_id") {
        lookup.insert("@ccbr_session_id".to_string(), session_id);
    }
    lookup
}

/// Build slot-scoped user option lookup for a session.
pub fn session_slot_user_option_lookup(session: &Session) -> HashMap<String, String> {
    if let Some(resolved) = session.slot_user_option_lookup.as_ref() {
        let normalized = normalize_option_map(resolved);
        if !normalized.is_empty() {
            return normalized;
        }
    }
    let mut lookup = HashMap::new();
    if let Some(agent_name) = session_data_text(session, "agent_name") {
        lookup.insert("@ccbr_agent".to_string(), agent_name);
    }
    if let Some(project_id) = session_data_text(session, "ccbr_project_id") {
        lookup.insert("@ccbr_project_id".to_string(), project_id);
    }
    if let Some(slot_key) = session_data_text(session, "ccbr_slot") {
        lookup.insert("@ccbr_slot".to_string(), slot_key);
    }
    if let Some(managed_by) = session_data_text(session, "ccbr_managed_by") {
        lookup.insert("@ccbr_managed_by".to_string(), managed_by);
    }
    lookup
}

/// Extract the pane title marker for a session.
pub fn session_pane_title_marker(session: &Session) -> Option<String> {
    session
        .pane_title_marker
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| session_data_text(session, "pane_title_marker"))
}

/// Extract the display title for a session.
pub fn session_display_title(session: &Session) -> Option<String> {
    if let Some(agent_name) = session_data_text(session, "agent_name") {
        return Some(agent_name);
    }
    let lookup = session_user_option_lookup(session);
    if let Some(agent_name) = lookup.get("@ccbr_agent") {
        return Some(agent_name.clone());
    }
    session_pane_title_marker(session)
}

/// Render a human-readable ownership error text.
pub fn ownership_error_text(ownership: &TmuxPaneOwnership, pane_id: Option<&str>) -> String {
    let target = pane_id
        .map(|s| s.trim())
        .or(ownership.pane_id.as_deref())
        .unwrap_or("<unknown>");
    if ownership.reason.as_deref() == Some("ownership-not-recorded") {
        return format!("Pane ownership not recorded for {}", target);
    }
    let expected = if ownership.expected_options.is_empty() {
        "none".to_string()
    } else {
        ownership
            .expected_options
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let actual = if ownership.actual_options.is_empty() {
        "none".to_string()
    } else {
        ownership
            .actual_options
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "Pane ownership mismatch for {}: expected [{}], actual [{}]",
        target, expected, actual
    )
}

fn expected_option_items(session: &Session) -> Vec<(String, String)> {
    let mut items: Vec<(String, String)> =
        session_user_option_lookup(session).into_iter().collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
}

fn inspect_described_pane(
    backend: &dyn SessionBackend,
    pane_id: &str,
    expected_items: &[(String, String)],
) -> Option<TmuxPaneOwnership> {
    let described = backend.describe_pane(
        pane_id,
        &expected_items
            .iter()
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>(),
    )?;
    let actual_title = described
        .get("pane_title")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let actual_items: Vec<(String, String)> = expected_items
        .iter()
        .map(|(name, _)| {
            let value = described
                .get(name)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            (name.clone(), value)
        })
        .collect();
    let state = if options_match(expected_items, &actual_items) {
        "owned"
    } else {
        "foreign"
    };
    Some(TmuxPaneOwnership {
        state: state.to_string(),
        pane_id: Some(pane_id.to_string()),
        pane_title: actual_title,
        expected_options: expected_items.to_vec(),
        actual_options: actual_items,
        reason: if state == "foreign" {
            Some("ownership-mismatch".to_string())
        } else {
            None
        },
    })
}

fn inspect_listed_panes(
    backend: &dyn SessionBackend,
    pane_id: &str,
    expected_items: &[(String, String)],
) -> Option<TmuxPaneOwnership> {
    let options: HashMap<String, String> = expected_items.iter().cloned().collect();
    let matches = listed_pane_matches(backend, &options)?;
    let state = if matches.contains(&pane_id.to_string()) {
        "owned"
    } else {
        "foreign"
    };
    Some(TmuxPaneOwnership {
        state: state.to_string(),
        pane_id: Some(pane_id.to_string()),
        pane_title: None,
        expected_options: expected_items.to_vec(),
        actual_options: if state == "owned" {
            expected_items.to_vec()
        } else {
            Vec::new()
        },
        reason: if state == "foreign" {
            Some("ownership-mismatch".to_string())
        } else {
            None
        },
    })
}

fn listed_pane_matches(
    backend: &dyn SessionBackend,
    options: &HashMap<String, String>,
) -> Option<Vec<String>> {
    Some(
        backend
            .list_panes_by_user_options(options)?
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

fn options_match(expected: &[(String, String)], actual: &[(String, String)]) -> bool {
    expected
        .iter()
        .zip(actual.iter())
        .all(|((_, exp), (_, act))| exp == act)
}

fn normalize_option_map(values: &HashMap<String, String>) -> HashMap<String, String> {
    let mut normalized = HashMap::new();
    for (raw_name, raw_value) in values {
        let name = raw_name.trim();
        let value = raw_value.trim();
        if name.is_empty() || value.is_empty() {
            continue;
        }
        let name = if name.starts_with('@') {
            name.to_string()
        } else {
            format!("@{}", name.trim_start_matches('@'))
        };
        normalized.insert(name, value.to_string());
    }
    normalized
}

fn session_data_text(session: &Session, key: &str) -> Option<String> {
    session
        .data
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestBackend;

    impl SessionBackend for TestBackend {
        fn socket_name(&self) -> Option<String> {
            None
        }
        fn socket_path(&self) -> Option<String> {
            None
        }
        fn is_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn is_tmux_pane_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn pane_exists(&self, _pane_id: &str) -> bool {
            true
        }
        fn describe_pane(
            &self,
            _pane_id: &str,
            _user_options: &[String],
        ) -> Option<HashMap<String, String>> {
            None
        }
        fn list_panes_by_user_options(
            &self,
            _options: &HashMap<String, String>,
        ) -> Option<Vec<String>> {
            None
        }
        fn set_pane_title(&self, _pane_id: &str, _title: &str) -> Result<(), String> {
            Ok(())
        }
        fn set_pane_user_option(
            &self,
            _pane_id: &str,
            _name: &str,
            _value: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_session_user_option_lookup() {
        let mut session = Session::default();
        session.data.insert(
            "agent_name".to_string(),
            serde_json::Value::String("claude-reviewer".to_string()),
        );
        session.data.insert(
            "ccbr_project_id".to_string(),
            serde_json::Value::String("proj-1".to_string()),
        );
        let lookup = session_user_option_lookup(&session);
        assert_eq!(
            lookup.get("@ccbr_agent"),
            Some(&"claude-reviewer".to_string())
        );
        assert_eq!(lookup.get("@ccbr_project_id"), Some(&"proj-1".to_string()));
    }

    #[test]
    fn test_inspect_tmux_pane_ownership_empty_pane() {
        let session = Session::default();
        let backend = TestBackend;
        let ownership = inspect_tmux_pane_ownership(&session, &backend, "");
        assert!(!ownership.is_owned());
        assert_eq!(ownership.reason, Some("pane-id-missing".to_string()));
    }

    #[test]
    fn test_ownership_error_text() {
        let ownership = TmuxPaneOwnership {
            state: "foreign".to_string(),
            pane_id: Some("%1".to_string()),
            pane_title: None,
            expected_options: vec![("@ccbr_agent".to_string(), "a1".to_string())],
            actual_options: vec![("@ccbr_agent".to_string(), "a2".to_string())],
            reason: Some("ownership-mismatch".to_string()),
        };
        let text = ownership_error_text(&ownership, None);
        assert!(text.contains("Pane ownership mismatch for %1"));
        assert!(text.contains("@ccbr_agent=a1"));
        assert!(text.contains("@ccbr_agent=a2"));
    }
}
