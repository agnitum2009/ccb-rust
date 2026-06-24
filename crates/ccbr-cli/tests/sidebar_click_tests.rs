use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ccbr_cli::sidebar_click::{
    focus_sidebar_click, sidebar_tree_targets, SidebarClick, SidebarClickClient,
};

const SAMPLE_VIEW_JSON: &str = r#"{
    "namespace": {"epoch": 7},
    "windows": [
        {"name": "main"},
        {"name": "work"},
        {"name": "review"}
    ],
    "agents": [
        {"name": "agent1", "window": "main"},
        {"name": "agent2", "window": "main"},
        {"name": "agent3", "window": "work"},
        {"name": "agent4", "window": "review"}
    ]
}"#;

type Calls = Arc<Mutex<Vec<(String, String, Option<i64>)>>>;

struct FakeClient {
    calls: Calls,
}

impl FakeClient {
    fn new() -> (Self, Calls) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
            },
            calls,
        )
    }
}

impl SidebarClickClient for FakeClient {
    fn project_view(&self, schema_version: i64) -> Result<serde_json::Value, String> {
        assert_eq!(schema_version, 1);
        Ok(serde_json::json!({
            "view": serde_json::from_str::<serde_json::Value>(SAMPLE_VIEW_JSON).unwrap()
        }))
    }

    fn project_focus_window(
        &self,
        window: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<serde_json::Value, String> {
        self.calls.lock().unwrap().push((
            "window".to_string(),
            window.to_string(),
            namespace_epoch,
        ));
        Ok(serde_json::Value::Null)
    }

    fn project_focus_agent(
        &self,
        agent: &str,
        namespace_epoch: Option<i64>,
    ) -> Result<serde_json::Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push(("agent".to_string(), agent.to_string(), namespace_epoch));
        Ok(serde_json::Value::Null)
    }
}

#[test]
fn test_sidebar_tree_targets_match_sidebar_render_order() {
    let view: serde_json::Value = serde_json::from_str(SAMPLE_VIEW_JSON).unwrap();
    assert_eq!(
        sidebar_tree_targets(&view),
        vec![
            ("window".to_string(), "main".to_string()),
            ("agent".to_string(), "agent1".to_string()),
            ("agent".to_string(), "agent2".to_string()),
            ("window".to_string(), "work".to_string()),
            ("agent".to_string(), "agent3".to_string()),
            ("window".to_string(), "review".to_string()),
            ("agent".to_string(), "agent4".to_string()),
        ]
    );
}

#[test]
fn test_sidebar_click_focuses_window_from_pane_relative_tmux_row() {
    let (client, calls) = FakeClient::new();
    let target = focus_sidebar_click(
        &SidebarClick {
            socket_path: PathBuf::from("/tmp/ccbd.sock"),
            mouse_y: 4,
            pane_top: 1,
            pane_height: 47,
        },
        |_path| client,
    )
    .unwrap();

    assert_eq!(target, Some("window:work".to_string()));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("window".to_string(), "work".to_string(), Some(7))]
    );
}

#[test]
fn test_sidebar_click_focuses_agent_from_second_agent_row() {
    let (client, calls) = FakeClient::new();
    let target = focus_sidebar_click(
        &SidebarClick {
            socket_path: PathBuf::from("/tmp/ccbd.sock"),
            mouse_y: 3,
            pane_top: 1,
            pane_height: 47,
        },
        |_path| client,
    )
    .unwrap();

    assert_eq!(target, Some("agent:agent2".to_string()));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("agent".to_string(), "agent2".to_string(), Some(7))]
    );
}

#[test]
fn test_sidebar_click_accepts_absolute_tmux_row_when_outside_pane_relative_range() {
    let (client, calls) = FakeClient::new();
    let target = focus_sidebar_click(
        &SidebarClick {
            socket_path: PathBuf::from("/tmp/ccbd.sock"),
            mouse_y: 52,
            pane_top: 48,
            pane_height: 47,
        },
        |_path| client,
    )
    .unwrap();

    assert_eq!(target, Some("window:work".to_string()));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("window".to_string(), "work".to_string(), Some(7))]
    );
}

#[test]
fn test_sidebar_click_ignores_title_border_and_empty_rows() {
    let (client1, calls1) = FakeClient::new();
    let title = focus_sidebar_click(
        &SidebarClick {
            socket_path: PathBuf::from("/tmp/ccbd.sock"),
            mouse_y: 0,
            pane_top: 1,
            pane_height: 47,
        },
        |_path| client1,
    )
    .unwrap();

    let (client2, calls2) = FakeClient::new();
    let empty = focus_sidebar_click(
        &SidebarClick {
            socket_path: PathBuf::from("/tmp/ccbd.sock"),
            mouse_y: 20,
            pane_top: 1,
            pane_height: 47,
        },
        |_path| client2,
    )
    .unwrap();

    assert_eq!(title, None);
    assert_eq!(empty, None);
    assert!(calls1.lock().unwrap().is_empty());
    assert!(calls2.lock().unwrap().is_empty());
}
