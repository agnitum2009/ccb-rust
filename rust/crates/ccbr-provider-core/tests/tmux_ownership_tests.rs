//! Mirrors Python `test/test_tmux_ownership_inspection.py`.

use std::collections::HashMap;

use ccbr_provider_core::session_binding::{Session, SessionBackend};
use ccbr_provider_core::tmux_ownership::inspect_tmux_pane_ownership;

#[derive(Debug)]
struct DescribingBackend {
    expected_pane_id: String,
    expected_options: Vec<String>,
    described: HashMap<String, String>,
}

impl SessionBackend for DescribingBackend {
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
        pane_id: &str,
        user_options: &[String],
    ) -> Option<HashMap<String, String>> {
        assert_eq!(pane_id, self.expected_pane_id);
        assert_eq!(user_options, &self.expected_options);
        Some(self.described.clone())
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
fn test_tmux_ownership_prefers_described_pane_match() {
    let mut session = Session::default();
    session
        .data
        .insert("agent_name".to_string(), serde_json::json!("agent1"));
    session
        .data
        .insert("ccbr_project_id".to_string(), serde_json::json!("proj_1"));
    session
        .data
        .insert("ccbr_session_id".to_string(), serde_json::json!("sess_1"));

    let mut described = HashMap::new();
    described.insert("pane_title".to_string(), "agent1".to_string());
    described.insert("@ccb_agent".to_string(), "agent1".to_string());
    described.insert("@ccb_project_id".to_string(), "proj_1".to_string());
    described.insert("@ccb_session_id".to_string(), "sess_1".to_string());

    let backend = DescribingBackend {
        expected_pane_id: "%12".to_string(),
        expected_options: vec![
            "@ccb_agent".to_string(),
            "@ccb_project_id".to_string(),
            "@ccb_session_id".to_string(),
        ],
        described,
    };

    let ownership = inspect_tmux_pane_ownership(&session, &backend, "%12");

    assert!(ownership.is_owned());
    assert_eq!(ownership.pane_title, Some("agent1".to_string()));
    assert_eq!(
        ownership.actual_options,
        vec![
            ("@ccb_agent".to_string(), "agent1".to_string()),
            ("@ccb_project_id".to_string(), "proj_1".to_string()),
            ("@ccb_session_id".to_string(), "sess_1".to_string()),
        ]
    );
}

#[derive(Debug)]
struct ListingBackend {
    expected_options: HashMap<String, String>,
    matches: Vec<String>,
}

impl SessionBackend for ListingBackend {
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
    fn list_panes_by_user_options(&self, options: &HashMap<String, String>) -> Option<Vec<String>> {
        assert_eq!(options, &self.expected_options);
        Some(self.matches.clone())
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
fn test_tmux_ownership_reports_foreign_when_listed_match_missing() {
    let mut session = Session::default();
    session
        .data
        .insert("agent_name".to_string(), serde_json::json!("agent1"));

    let mut expected_options = HashMap::new();
    expected_options.insert("@ccb_agent".to_string(), "agent1".to_string());

    let backend = ListingBackend {
        expected_options,
        matches: vec!["%2".to_string(), "%3".to_string()],
    };

    let ownership = inspect_tmux_pane_ownership(&session, &backend, "%9");

    assert!(!ownership.is_owned());
    assert_eq!(ownership.state, "foreign");
    assert_eq!(ownership.reason, Some("ownership-mismatch".to_string()));
}
