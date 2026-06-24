use std::collections::HashSet;

use ccbr_terminal::layouts::{create_tmux_auto_layout, TmuxLayoutBackend};

struct FakeLayoutBackend {
    current_pane: Option<String>,
    alive_sessions: HashSet<String>,
    split_calls: std::sync::Mutex<Vec<(String, String, u32)>>,
    title_calls: std::sync::Mutex<Vec<(String, String)>>,
    tmux_calls: std::sync::Mutex<Vec<(Vec<String>, bool, bool)>>,
    seq: std::sync::Mutex<std::vec::IntoIter<String>>,
}

impl FakeLayoutBackend {
    fn new(current_pane: Option<&str>, alive_sessions: &[&str]) -> Self {
        Self {
            current_pane: current_pane.map(|s| s.to_string()),
            alive_sessions: alive_sessions.iter().map(|s| s.to_string()).collect(),
            split_calls: std::sync::Mutex::new(Vec::new()),
            title_calls: std::sync::Mutex::new(Vec::new()),
            tmux_calls: std::sync::Mutex::new(Vec::new()),
            seq: std::sync::Mutex::new(
                vec![
                    "%1".to_string(),
                    "%2".to_string(),
                    "%3".to_string(),
                    "%4".to_string(),
                    "%5".to_string(),
                ]
                .into_iter(),
            ),
        }
    }
}

impl TmuxLayoutBackend for FakeLayoutBackend {
    fn get_current_pane_id(&self) -> anyhow::Result<String> {
        match &self.current_pane {
            Some(p) => Ok(p.clone()),
            None => Err(anyhow::anyhow!("no current pane")),
        }
    }

    fn is_alive(&self, pane_id: &str) -> bool {
        self.alive_sessions.contains(pane_id)
    }

    fn create_pane(
        &self,
        _cmd: &str,
        _cwd: &str,
        _direction: &str,
        _percent: u32,
        _parent_pane: Option<&str>,
    ) -> anyhow::Result<String> {
        Ok("%created".to_string())
    }

    fn split_pane(
        &self,
        parent_pane_id: &str,
        direction: &str,
        percent: u32,
    ) -> anyhow::Result<String> {
        self.split_calls.lock().unwrap().push((
            parent_pane_id.to_string(),
            direction.to_string(),
            percent,
        ));
        Ok(self
            .seq
            .lock()
            .unwrap()
            .next()
            .unwrap_or_else(|| "%0".to_string()))
    }

    fn set_pane_title(&self, pane_id: &str, title: &str) {
        self.title_calls
            .lock()
            .unwrap()
            .push((pane_id.to_string(), title.to_string()));
    }

    fn set_pane_user_option(&self, _pane_id: &str, _name: &str, _value: &str) {}

    fn set_pane_style(
        &self,
        _pane_id: &str,
        _border_style: Option<&str>,
        _active_border_style: Option<&str>,
    ) {
    }

    fn tmux_run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<String> {
        self.tmux_calls.lock().unwrap().push((
            args.iter().map(|s| s.to_string()).collect(),
            check,
            capture,
        ));
        if args.len() >= 2 && args[0] == "list-panes" && args[1] == "-t" {
            return Ok("%root-detached\n".to_string());
        }
        Ok("".to_string())
    }
}

#[test]
fn test_create_tmux_auto_layout_uses_current_pane_when_available() {
    let backend = FakeLayoutBackend::new(Some("%root"), &[]);
    let result = create_tmux_auto_layout(
        &["agent1".to_string(), "agent2".to_string()],
        "/tmp",
        &backend,
        None,
        None,
        50,
        true,
        "M",
        None,
        false,
    )
    .unwrap();
    assert_eq!(result.panes.get("agent1"), Some(&"%root".to_string()));
    assert_eq!(result.panes.get("agent2"), Some(&"%1".to_string()));
    assert_eq!(result.created_panes, vec!["%1"]);
    assert!(!result.needs_attach);
    assert_eq!(
        *backend.split_calls.lock().unwrap(),
        vec![("%root".to_string(), "right".to_string(), 50)]
    );
    assert_eq!(
        *backend.title_calls.lock().unwrap(),
        vec![
            ("%root".to_string(), "M-agent1".to_string()),
            ("%1".to_string(), "M-agent2".to_string()),
        ]
    );
}

#[test]
fn test_create_tmux_auto_layout_allocates_detached_session_when_outside_tmux() {
    let backend = FakeLayoutBackend::new(None, &[]);
    let result = create_tmux_auto_layout(
        &["agent1".to_string()],
        "/tmp/demo",
        &backend,
        None,
        None,
        50,
        true,
        "CCB",
        Some("ccbr-demo-1"),
        false,
    )
    .unwrap();
    assert_eq!(
        result.panes.get("agent1"),
        Some(&"%root-detached".to_string())
    );
    assert_eq!(result.root_pane_id, "%root-detached");
    assert_eq!(result.created_panes, vec!["%root-detached"]);
    assert!(result.needs_attach);
    let tmux_calls = backend.tmux_calls.lock().unwrap();
    assert_eq!(tmux_calls.len(), 2);
    assert_eq!(
        tmux_calls[0],
        (
            vec![
                "new-session".to_string(),
                "-d".to_string(),
                "-s".to_string(),
                "ccbr-demo-1".to_string(),
                "-c".to_string(),
                "/tmp/demo".to_string(),
                "sh".to_string(),
                "-lc".to_string(),
                "while :; do sleep 3600; done".to_string(),
            ],
            true,
            false,
        )
    );
    assert_eq!(
        tmux_calls[1],
        (
            vec![
                "list-panes".to_string(),
                "-t".to_string(),
                "ccbr-demo-1".to_string(),
                "-F".to_string(),
                "#{pane_id}".to_string(),
            ],
            true,
            true,
        )
    );
}

#[test]
fn test_create_auto_layout_topologies() {
    use std::sync::Mutex;

    struct SeqBackend {
        split_calls: Mutex<Vec<(String, String)>>,
        title_calls: Mutex<Vec<(String, String)>>,
        seq: Mutex<u32>,
    }

    impl TmuxLayoutBackend for SeqBackend {
        fn get_current_pane_id(&self) -> anyhow::Result<String> {
            Ok("%root".to_string())
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
            Ok("%created".to_string())
        }
        fn split_pane(
            &self,
            parent_pane_id: &str,
            direction: &str,
            _percent: u32,
        ) -> anyhow::Result<String> {
            self.split_calls
                .lock()
                .unwrap()
                .push((parent_pane_id.to_string(), direction.to_string()));
            let n = *self.seq.lock().unwrap();
            *self.seq.lock().unwrap() += 1;
            Ok(format!("%r{n}"))
        }
        fn set_pane_title(&self, pane_id: &str, title: &str) {
            self.title_calls
                .lock()
                .unwrap()
                .push((pane_id.to_string(), title.to_string()));
        }
        fn set_pane_user_option(&self, _pane_id: &str, _name: &str, _value: &str) {}
        fn set_pane_style(
            &self,
            _pane_id: &str,
            _border_style: Option<&str>,
            _active_border_style: Option<&str>,
        ) {
        }
        fn tmux_run(&self, _args: &[&str], _check: bool, _capture: bool) -> anyhow::Result<String> {
            Ok("".to_string())
        }
    }

    let backend = SeqBackend {
        split_calls: Mutex::new(Vec::new()),
        title_calls: Mutex::new(Vec::new()),
        seq: Mutex::new(1),
    };

    let r2 = create_tmux_auto_layout(
        &["codex".to_string(), "gemini".to_string()],
        "/tmp",
        &backend,
        None,
        None,
        50,
        true,
        "M",
        None,
        true,
    )
    .unwrap();
    assert_eq!(r2.panes.get("codex"), Some(&"%root".to_string()));
    assert_eq!(r2.panes.get("gemini"), Some(&"%r1".to_string()));
    assert_eq!(
        *backend.split_calls.lock().unwrap(),
        vec![("%root".to_string(), "right".to_string())]
    );
    {
        let titles: Vec<String> = backend
            .title_calls
            .lock()
            .unwrap()
            .iter()
            .map(|(_, t)| t.clone())
            .collect();
        assert!(titles.contains(&"M-codex".to_string()));
        assert!(titles.contains(&"M-gemini".to_string()));
    }

    backend.split_calls.lock().unwrap().clear();

    let r3 = create_tmux_auto_layout(
        &[
            "codex".to_string(),
            "gemini".to_string(),
            "opencode".to_string(),
        ],
        "/tmp",
        &backend,
        None,
        None,
        50,
        true,
        "M",
        None,
        true,
    )
    .unwrap();
    assert_eq!(r3.panes.get("codex"), Some(&"%root".to_string()));
    assert_eq!(r3.panes.get("gemini"), Some(&"%r2".to_string()));
    assert_eq!(r3.panes.get("opencode"), Some(&"%r3".to_string()));
    assert_eq!(
        *backend.split_calls.lock().unwrap(),
        vec![
            ("%root".to_string(), "right".to_string()),
            ("%r2".to_string(), "bottom".to_string()),
        ]
    );

    backend.split_calls.lock().unwrap().clear();

    let r4 = create_tmux_auto_layout(
        &[
            "codex".to_string(),
            "gemini".to_string(),
            "opencode".to_string(),
            "x".to_string(),
        ],
        "/tmp",
        &backend,
        None,
        None,
        50,
        true,
        "M",
        None,
        true,
    )
    .unwrap();
    assert_eq!(r4.panes.get("codex"), Some(&"%root".to_string()));
    assert_eq!(r4.panes.get("gemini"), Some(&"%r4".to_string()));
    assert_eq!(r4.panes.get("opencode"), Some(&"%r5".to_string()));
    assert_eq!(r4.panes.get("x"), Some(&"%r6".to_string()));
    assert_eq!(
        *backend.split_calls.lock().unwrap(),
        vec![
            ("%root".to_string(), "right".to_string()),
            ("%root".to_string(), "bottom".to_string()),
            ("%r4".to_string(), "bottom".to_string()),
        ]
    );
}

#[test]
fn test_create_tmux_auto_layout_reuses_existing_session() {
    let backend = FakeLayoutBackend::new(None, &["ccbr-demo-2"]);
    let result = create_tmux_auto_layout(
        &[
            "agent1".to_string(),
            "agent2".to_string(),
            "agent3".to_string(),
        ],
        "/tmp/demo",
        &backend,
        None,
        Some("ccbr-demo-2"),
        50,
        true,
        "CCB",
        None,
        true,
    )
    .unwrap();
    assert_eq!(result.root_pane_id, "%root-detached");
    assert!(!result.needs_attach);
    assert_eq!(
        *backend.split_calls.lock().unwrap(),
        vec![
            ("%root-detached".to_string(), "right".to_string(), 50),
            ("%1".to_string(), "bottom".to_string(), 50),
        ]
    );
    let tmux_calls = backend.tmux_calls.lock().unwrap();
    assert_eq!(tmux_calls.len(), 1);
    assert_eq!(
        tmux_calls[0],
        (
            vec![
                "list-panes".to_string(),
                "-t".to_string(),
                "ccbr-demo-2".to_string(),
                "-F".to_string(),
                "#{pane_id}".to_string(),
            ],
            true,
            true,
        )
    );
}
