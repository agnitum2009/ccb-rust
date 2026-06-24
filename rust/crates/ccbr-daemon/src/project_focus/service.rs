pub struct ProjectFocusService {
    focused_window: Option<String>,
    focused_agent: Option<String>,
}

impl ProjectFocusService {
    pub fn new() -> Self {
        Self {
            focused_window: None,
            focused_agent: None,
        }
    }

    pub fn focus_window(
        &mut self,
        window: &str,
        namespace_epoch: Option<u64>,
    ) -> serde_json::Value {
        self.focused_window = Some(window.into());
        serde_json::json!({
            "status": "ok",
            "window": window,
            "namespace_epoch": namespace_epoch,
        })
    }

    pub fn focus_agent(&mut self, agent: &str, namespace_epoch: Option<u64>) -> serde_json::Value {
        self.focused_agent = Some(agent.into());
        serde_json::json!({
            "status": "ok",
            "agent": agent,
            "namespace_epoch": namespace_epoch,
        })
    }

    pub fn focused_window(&self) -> Option<&str> {
        self.focused_window.as_deref()
    }

    pub fn focused_agent(&self) -> Option<&str> {
        self.focused_agent.as_deref()
    }
}

impl Default for ProjectFocusService {
    fn default() -> Self {
        Self::new()
    }
}
