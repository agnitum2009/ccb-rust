#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::tmux;

/// Layout node in a tmux window tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayoutNode {
    Split {
        direction: SplitDirection,
        children: Vec<LayoutNode>,
        #[serde(default)]
        percentages: Vec<u32>,
    },
    Leaf {
        command: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// Generate tmux layout commands from a layout tree.
pub fn generate_layout_commands(layout: &LayoutNode, _parent_pane: Option<&str>) -> Vec<String> {
    match layout {
        LayoutNode::Leaf { command, cwd, .. } => {
            let c = cwd.as_deref().unwrap_or(".");
            vec![format!("split-window -c {} {}", c, command)]
        }
        LayoutNode::Split { children, .. } => {
            let mut cmds = Vec::new();
            for child in children {
                cmds.extend(generate_layout_commands(child, _parent_pane));
            }
            cmds
        }
    }
}

/// Backend required by layout operations.
pub trait TmuxLayoutBackend: Send + Sync {
    fn get_current_pane_id(&self) -> anyhow::Result<String>;
    fn is_alive(&self, pane_id: &str) -> bool;
    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        direction: &str,
        percent: u32,
        parent_pane: Option<&str>,
    ) -> anyhow::Result<String>;
    fn split_pane(
        &self,
        parent_pane_id: &str,
        direction: &str,
        percent: u32,
    ) -> anyhow::Result<String>;
    fn set_pane_title(&self, pane_id: &str, title: &str);
    fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str);
    fn set_pane_style(
        &self,
        pane_id: &str,
        border_style: Option<&str>,
        active_border_style: Option<&str>,
    );
    fn tmux_run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<String>;
}

/// Result of creating a tmux auto layout.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub panes: HashMap<String, String>,
    pub root_pane_id: String,
    pub needs_attach: bool,
    pub created_panes: Vec<String>,
}

/// Create an automatic tmux layout for a list of providers.
pub fn create_tmux_auto_layout<B: TmuxLayoutBackend>(
    providers: &[String],
    cwd: &str,
    backend: &B,
    root_pane_id: Option<&str>,
    tmux_session_name: Option<&str>,
    percent: u32,
    set_markers: bool,
    marker_prefix: &str,
    detached_session_name: Option<&str>,
    inside_tmux: bool,
) -> anyhow::Result<LayoutResult> {
    if providers.is_empty() {
        return Err(anyhow::anyhow!("providers must not be empty"));
    }
    if providers.len() > 4 {
        return Err(anyhow::anyhow!("providers max is 4 for auto layout"));
    }

    let mut panes: HashMap<String, String> = HashMap::new();
    let (root, needs_attach, mut created) = resolve_root_pane(
        backend,
        cwd,
        root_pane_id,
        tmux_session_name,
        detached_session_name,
        inside_tmux,
    )?;

    panes.insert(providers[0].clone(), root.clone());
    let mark = build_marker(backend, set_markers, marker_prefix);
    mark(&providers[0], &root);

    if providers.len() == 1 {
        return Ok(build_layout_result(panes, root, needs_attach, created));
    }

    let pct = percent.clamp(1, 99);
    build_split_layout(
        backend,
        providers,
        &mut panes,
        &mut created,
        &root,
        pct,
        &mark,
    )?;
    Ok(build_layout_result(panes, root, needs_attach, created))
}

fn resolve_root_pane<B: TmuxLayoutBackend>(
    backend: &B,
    cwd: &str,
    root_pane_id: Option<&str>,
    tmux_session_name: Option<&str>,
    detached_session_name: Option<&str>,
    inside_tmux: bool,
) -> anyhow::Result<(String, bool, Vec<String>)> {
    if let Some(root) = root_pane_id {
        return Ok((root.to_string(), false, Vec::new()));
    }
    if let Ok(root) = backend.get_current_pane_id() {
        return Ok((root, false, Vec::new()));
    }
    let root = detached_root_pane(
        backend,
        cwd,
        tmux_session_name.or(detached_session_name).unwrap_or(""),
    )?;
    Ok((root.clone(), !inside_tmux, vec![root]))
}

fn detached_root_pane<B: TmuxLayoutBackend>(
    backend: &B,
    cwd: &str,
    session_name: &str,
) -> anyhow::Result<String> {
    let root = if !session_name.is_empty() {
        if !backend.is_alive(session_name) {
            let mut args = vec![
                "new-session".to_string(),
                "-d".to_string(),
                "-s".to_string(),
                session_name.to_string(),
                "-c".to_string(),
                cwd.to_string(),
            ];
            args.extend(tmux::pane_placeholder_argv());
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            backend.tmux_run(&args_ref, true, false)?;
        }
        let output = backend.tmux_run(
            &["list-panes", "-t", session_name, "-F", "#{pane_id}"],
            true,
            true,
        )?;
        first_pane_id(&output)
    } else {
        backend.create_pane("", cwd, "right", 50, None)?
    };
    if root.starts_with('%') {
        Ok(root)
    } else {
        Err(anyhow::anyhow!("failed to allocate tmux root pane"))
    }
}

fn first_pane_id(stdout: &str) -> String {
    stdout
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

fn build_marker<'a, B: TmuxLayoutBackend>(
    backend: &'a B,
    enabled: bool,
    marker_prefix: &'a str,
) -> Box<dyn Fn(&str, &str) + 'a> {
    Box::new(move |provider: &str, pane_id: &str| {
        if enabled {
            backend.set_pane_title(pane_id, &format!("{marker_prefix}-{provider}"));
        }
    })
}

fn build_split_layout<B: TmuxLayoutBackend>(
    backend: &B,
    providers: &[String],
    panes: &mut HashMap<String, String>,
    created: &mut Vec<String>,
    root: &str,
    percent: u32,
    mark: &dyn Fn(&str, &str),
) -> anyhow::Result<()> {
    match providers.len() {
        2 => {
            assign_pane(
                backend,
                &providers[1],
                panes,
                created,
                root,
                "right",
                percent,
                mark,
            )?;
        }
        3 => {
            let right_top = assign_pane(
                backend,
                &providers[1],
                panes,
                created,
                root,
                "right",
                percent,
                mark,
            )?;
            assign_pane(
                backend,
                &providers[2],
                panes,
                created,
                &right_top,
                "bottom",
                percent,
                mark,
            )?;
        }
        4 => {
            let right_top = assign_pane(
                backend,
                &providers[1],
                panes,
                created,
                root,
                "right",
                percent,
                mark,
            )?;
            assign_pane(
                backend,
                &providers[2],
                panes,
                created,
                root,
                "bottom",
                percent,
                mark,
            )?;
            assign_pane(
                backend,
                &providers[3],
                panes,
                created,
                &right_top,
                "bottom",
                percent,
                mark,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn assign_pane<B: TmuxLayoutBackend>(
    backend: &B,
    provider: &str,
    panes: &mut HashMap<String, String>,
    created: &mut Vec<String>,
    parent: &str,
    direction: &str,
    percent: u32,
    mark: &dyn Fn(&str, &str),
) -> anyhow::Result<String> {
    let pane_id = backend.split_pane(parent, direction, percent)?;
    created.push(pane_id.clone());
    panes.insert(provider.to_string(), pane_id.clone());
    mark(provider, &pane_id);
    Ok(pane_id)
}

fn build_layout_result(
    panes: HashMap<String, String>,
    root: String,
    needs_attach: bool,
    created: Vec<String>,
) -> LayoutResult {
    LayoutResult {
        panes,
        root_pane_id: root,
        needs_attach,
        created_panes: created,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeLayoutBackend {
        current_pane: Option<String>,
        alive_sessions: std::collections::HashSet<String>,
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
    fn test_layout_leaf_serde() {
        let leaf = LayoutNode::Leaf {
            command: "bash".into(),
            cwd: Some("/tmp".into()),
            label: None,
        };
        let json = serde_json::to_string(&leaf).unwrap();
        assert!(json.contains("\"type\":\"leaf\""));
        let _: LayoutNode = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_generate_layout_commands() {
        let leaf = LayoutNode::Leaf {
            command: "vim".into(),
            cwd: Some("/home".into()),
            label: None,
        };
        let cmds = generate_layout_commands(&leaf, None);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("vim"));
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
        let calls = backend.tmux_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            (
                vec![
                    "new-session",
                    "-d",
                    "-s",
                    "ccbr-demo-1",
                    "-c",
                    "/tmp/demo",
                    "sh",
                    "-lc",
                    "while :; do sleep 3600; done",
                ]
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
                true,
                false,
            )
        );
        assert_eq!(
            calls[1],
            (
                vec!["list-panes", "-t", "ccbr-demo-1", "-F", "#{pane_id}"]
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
                true,
                true,
            )
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
        let calls = backend.tmux_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            (
                vec!["list-panes", "-t", "ccbr-demo-2", "-F", "#{pane_id}"]
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
                true,
                true,
            )
        );
        assert_eq!(
            *backend.split_calls.lock().unwrap(),
            vec![
                ("%root-detached".to_string(), "right".to_string(), 50),
                ("%1".to_string(), "bottom".to_string(), 50),
            ]
        );
    }
}
