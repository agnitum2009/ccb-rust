#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use std::collections::HashMap;

/// Visual styling for a CCBR pane.
#[derive(Debug, Clone)]
pub struct TmuxPaneVisual {
    pub label_style: String,
    pub border_style: String,
    pub active_border_style: String,
}

/// Compute visual style for a pane.
pub fn pane_visual(
    project_id: &str,
    slot_key: &str,
    order_index: Option<i32>,
    is_cmd: bool,
    role: &str,
) -> TmuxPaneVisual {
    crate::theme::pane_visual(
        Some(project_id),
        Some(slot_key),
        order_index.map(|i| i as usize),
        is_cmd,
        Some(role),
        None,
        None,
    )
}

/// Apply CCBR identity metadata to a tmux pane.
pub fn apply_ccbr_pane_identity<B>(
    backend: &B,
    pane_id: &str,
    title: &str,
    agent_label: &str,
    project_id: &str,
    order_index: Option<i32>,
    is_cmd: bool,
    role: Option<&str>,
    slot_key: Option<&str>,
    window_name: Option<&str>,
    sidebar_instance: Option<&str>,
    session_id: Option<&str>,
    namespace_epoch: Option<i64>,
    managed_by: Option<&str>,
) where
    B: crate::layouts::TmuxLayoutBackend,
{
    let role_text = role
        .map(|r| r.trim())
        .filter(|r| !r.is_empty())
        .unwrap_or(if is_cmd { "cmd" } else { "agent" });
    let slot = slot_key.unwrap_or(title);
    let visual = pane_visual(project_id, slot, order_index, is_cmd, role_text);

    backend.set_pane_title(pane_id, title);
    let options: Vec<(&str, String)> = vec![
        ("@ccb_label_style", visual.label_style.clone()),
        ("@ccb_border_style", visual.border_style.clone()),
        (
            "@ccb_active_border_style",
            visual.active_border_style.clone(),
        ),
        ("@ccb_agent", agent_label.to_string()),
        ("@ccb_role", role_text.to_string()),
        ("@ccb_slot", slot.to_string()),
        ("@ccb_project_id", project_id.to_string()),
    ];
    let mut opts: HashMap<&str, String> = options.into_iter().collect();
    if let Some(window) = window_name {
        opts.insert("@ccb_window", window.trim().to_string());
    }
    if let Some(sidebar) = sidebar_instance {
        opts.insert("@ccb_sidebar_instance", sidebar.trim().to_string());
    }
    if let Some(session) = session_id {
        opts.insert("@ccb_session_id", session.trim().to_string());
    }
    if let Some(epoch) = namespace_epoch {
        opts.insert("@ccb_namespace_epoch", epoch.to_string());
    }
    let managed = managed_by.unwrap_or("ccbrd");
    opts.insert("@ccb_managed_by", managed.trim().to_string());

    for (name, value) in opts {
        backend.set_pane_user_option(pane_id, name, &value);
    }
    backend.set_pane_style(
        pane_id,
        Some(&visual.border_style),
        Some(&visual.active_border_style),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layouts::TmuxLayoutBackend;

    struct FakeBackend {
        titles: std::sync::Mutex<Vec<(String, String)>>,
        options: std::sync::Mutex<Vec<(String, String, String)>>,
        styles: std::sync::Mutex<Vec<(String, Option<String>, Option<String>)>>,
    }

    impl FakeBackend {
        fn new() -> Self {
            Self {
                titles: std::sync::Mutex::new(Vec::new()),
                options: std::sync::Mutex::new(Vec::new()),
                styles: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl TmuxLayoutBackend for FakeBackend {
        fn get_current_pane_id(&self) -> anyhow::Result<String> {
            Ok("%0".to_string())
        }
        fn is_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn create_pane(
            &self,
            _cmd: &str,
            _cwd: &str,
            _direction: &str,
            _percent: u32,
            _parent_pane: Option<&str>,
        ) -> anyhow::Result<String> {
            Ok("%0".to_string())
        }
        fn split_pane(
            &self,
            _parent_pane_id: &str,
            _direction: &str,
            _percent: u32,
        ) -> anyhow::Result<String> {
            Ok("%0".to_string())
        }
        fn set_pane_title(&self, pane_id: &str, title: &str) {
            self.titles
                .lock()
                .unwrap()
                .push((pane_id.to_string(), title.to_string()));
        }
        fn tmux_run(&self, _args: &[&str], _check: bool, _capture: bool) -> anyhow::Result<String> {
            Ok("".to_string())
        }
        fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) {
            self.options.lock().unwrap().push((
                pane_id.to_string(),
                name.to_string(),
                value.to_string(),
            ));
        }
        fn set_pane_style(
            &self,
            pane_id: &str,
            border_style: Option<&str>,
            active_border_style: Option<&str>,
        ) {
            self.styles.lock().unwrap().push((
                pane_id.to_string(),
                border_style.map(|s| s.to_string()),
                active_border_style.map(|s| s.to_string()),
            ));
        }
    }

    #[test]
    fn test_apply_ccbr_pane_identity_sets_title_and_options() {
        let backend = FakeBackend::new();
        apply_ccbr_pane_identity(
            &backend,
            "%1",
            "Claude",
            "claude-agent",
            "proj-42",
            Some(1),
            false,
            None,
            Some("slot-a"),
            Some("main"),
            None,
            Some("sess-1"),
            Some(1700000000),
            None,
        );
        assert_eq!(
            *backend.titles.lock().unwrap(),
            vec![("%1".to_string(), "Claude".to_string())]
        );
        let options = backend.options.lock().unwrap();
        assert!(options
            .iter()
            .any(|(_, k, v)| k == "@ccb_agent" && v == "claude-agent"));
        assert!(options
            .iter()
            .any(|(_, k, v)| k == "@ccb_project_id" && v == "proj-42"));
        assert!(options
            .iter()
            .any(|(_, k, v)| k == "@ccb_managed_by" && v == "ccbrd"));
    }
}
