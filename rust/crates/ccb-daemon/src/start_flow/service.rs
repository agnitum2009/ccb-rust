use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::provider_launcher::{LaunchContext, ProviderLauncher};
use crate::services::project_namespace::{NamespaceWindow, ProjectNamespace};
use crate::services::registry::AgentRegistry;
use crate::terminal_adapter::DaemonLayoutBackend;
use ccb_agents::models::WindowSpec;
use ccb_terminal::layouts::TmuxLayoutBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartFlowResult {
    pub status: String,
    pub agent_results: Vec<StartAgentResult>,
    pub actions_taken: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartAgentResult {
    pub agent_name: String,
    pub status: String,
    pub reason: Option<String>,
    pub pane_id: Option<String>,
}

/// Backend mode for the start flow.
#[derive(Debug, Clone)]
pub enum StartFlowMode {
    /// Use the real tmux backend via `ccb-terminal`.
    Tmux,
    /// Return synthetic pane ids without touching tmux.
    Stub,
}

pub struct StartFlowService {
    mode: StartFlowMode,
    stub_pane_counter: AtomicUsize,
}

impl StartFlowService {
    pub fn new(mode: StartFlowMode) -> Self {
        Self {
            mode,
            stub_pane_counter: AtomicUsize::new(0),
        }
    }

    pub fn with_tmux() -> Self {
        Self::new(StartFlowMode::Tmux)
    }

    pub fn with_stub() -> Self {
        Self::new(StartFlowMode::Stub)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        project_root: &Utf8Path,
        project_id: &str,
        tmux_socket_path: &str,
        tmux_session_name: &str,
        agent_names: &[String],
        registry: &AgentRegistry,
        restore: bool,
        auto_permission: bool,
        namespace_agent_panes: Option<&HashMap<String, String>>,
        config_windows: Option<Vec<WindowSpec>>,
    ) -> Result<(StartFlowResult, ProjectNamespace), String> {
        if agent_names.is_empty() {
            return Err("agent_names must not be empty".to_string());
        }
        let mut actions_taken = vec!["start_flow_executed".to_string()];
        if restore {
            actions_taken.push("restore_attempted".to_string());
        }
        if auto_permission {
            actions_taken.push("auto_permission_enabled".to_string());
        }

        let reused_panes: HashMap<String, String> = namespace_agent_panes
            .map(|m| {
                m.iter()
                    .filter(|(name, _)| agent_names.contains(name))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let reuse_all = agent_names
            .iter()
            .all(|name| reused_panes.contains_key(name));

        let (agent_panes, agent_results, root_pane_id) = match self.mode {
            StartFlowMode::Tmux => {
                let backend = DaemonLayoutBackend::new(tmux_socket_path);
                let mut all_panes: HashMap<String, String> = HashMap::new();
                let mut results = Vec::new();
                let mut root_pane: Option<String> = None;

                if reuse_all {
                    all_panes = reused_panes;
                    let names = agent_names.to_vec();
                    actions_taken.push(format!("use_namespace_topology:{}", names.join(",")));
                    // Best-effort root pane: first agent pane's session root.
                    if let Some(first_pane) = all_panes.values().next() {
                        root_pane = backend
                            .tmux_run(
                                &["display-message", "-p", "-t", first_pane, "#{pane_id}"],
                                false,
                                true,
                            )
                            .ok()
                            .map(|s| s.trim().to_string())
                            .filter(|s| s.starts_with('%'));
                    }
                } else if agent_names.len() <= 4 {
                    let launch_targets: Vec<_> = agent_names
                        .iter()
                        .filter(|name| !reused_panes.contains_key(*name))
                        .cloned()
                        .collect();
                    if !launch_targets.is_empty() {
                        actions_taken
                            .push(format!("prepare_tmux_layout:{}", launch_targets.join(",")));
                    }
                    // Only create panes for agents that are not being reused.
                    let layout = ccb_terminal::layouts::create_tmux_auto_layout(
                        &launch_targets,
                        project_root.as_str(),
                        &backend,
                        None,
                        Some(tmux_session_name),
                        50,
                        true,
                        "CCB",
                        Some(tmux_session_name),
                        false,
                    )
                    .map_err(|e| format!("layout creation failed: {e}"))?;
                    all_panes = layout.panes;
                    for (name, pane) in &reused_panes {
                        all_panes.insert(name.clone(), pane.clone());
                    }
                    root_pane = Some(layout.root_pane_id);
                } else {
                    // Auto layout supports at most 4 panes per session.
                    // Fall back to one detached session per agent.
                    for agent_name in agent_names {
                        let session = format!("{tmux_session_name}-{agent_name}");
                        let layout = ccb_terminal::layouts::create_tmux_auto_layout(
                            std::slice::from_ref(agent_name),
                            project_root.as_str(),
                            &backend,
                            None,
                            Some(&session),
                            50,
                            true,
                            "CCB",
                            Some(&session),
                            false,
                        )
                        .map_err(|e| format!("layout creation failed for {agent_name}: {e}"))?;
                        for (name, pane) in &layout.panes {
                            all_panes.insert(name.clone(), pane.clone());
                        }
                    }
                    for (name, pane) in &reused_panes {
                        all_panes.insert(name.clone(), pane.clone());
                    }
                }

                let launcher = ProviderLauncher::new();
                for agent_name in agent_names {
                    let pane_id = all_panes.get(agent_name).cloned();
                    let (status, reason) = if let Some(pane_id) = pane_id.as_deref() {
                        let entry = registry.get(agent_name);
                        let provider = entry.map(|e| e.provider.as_str()).unwrap_or("");
                        let workspace_path = entry
                            .and_then(|e| e.workspace_path.as_deref())
                            .unwrap_or(project_root.as_str());
                        if provider.is_empty() {
                            (
                                "failed".to_string(),
                                Some("no provider configured".to_string()),
                            )
                        } else {
                            let ctx = LaunchContext {
                                provider,
                                agent_name,
                                project_id,
                                project_root: project_root.as_str(),
                                workspace_path,
                                pane_id,
                                socket_path: tmux_socket_path,
                                restore,
                                command_template: None,
                                startup_args: &[],
                                auto_permission,
                            };
                            match launcher.launch(&ctx) {
                                Ok(_) => ("started".to_string(), None),
                                Err(e) => ("failed".to_string(), Some(e)),
                            }
                        }
                    } else {
                        ("failed".to_string(), Some("pane not allocated".to_string()))
                    };
                    results.push(StartAgentResult {
                        agent_name: agent_name.clone(),
                        status,
                        reason,
                        pane_id,
                    });
                }
                (all_panes, results, root_pane)
            }
            StartFlowMode::Stub => {
                let mut panes = std::collections::HashMap::new();
                let mut results = Vec::new();
                if reuse_all {
                    panes = reused_panes;
                    let names = agent_names.to_vec();
                    actions_taken.push(format!("use_namespace_topology:{}", names.join(",")));
                    for agent_name in agent_names {
                        if let Some(pane_id) = panes.get(agent_name).cloned() {
                            results.push(StartAgentResult {
                                agent_name: agent_name.clone(),
                                status: "started".to_string(),
                                reason: None,
                                pane_id: Some(pane_id),
                            });
                        }
                    }
                } else {
                    let launch_targets: Vec<_> = agent_names
                        .iter()
                        .filter(|name| !reused_panes.contains_key(*name))
                        .cloned()
                        .collect();
                    if !launch_targets.is_empty() {
                        actions_taken
                            .push(format!("prepare_tmux_layout:{}", launch_targets.join(",")));
                    }
                    for agent_name in agent_names {
                        let pane_id = if let Some(pane) = reused_panes.get(agent_name) {
                            pane.clone()
                        } else {
                            let id = self
                                .stub_pane_counter
                                .fetch_add(1, Ordering::SeqCst)
                                .to_string();
                            format!("%{id}")
                        };
                        panes.insert(agent_name.clone(), pane_id.clone());
                        results.push(StartAgentResult {
                            agent_name: agent_name.clone(),
                            status: "started".to_string(),
                            reason: None,
                            pane_id: Some(pane_id),
                        });
                    }
                }
                (panes, results, None)
            }
        };

        // Active panes include every allocated agent pane plus the root pane when known.
        let mut active_panes: Vec<String> = agent_panes.values().cloned().collect();
        if let Some(root) = root_pane_id {
            if !active_panes.contains(&root) {
                active_panes.push(root);
            }
        }

        // Build namespace windows from config or create default topology
        let namespace_windows = if let Some(windows) = config_windows {
            // Build windows from config
            windows
                .into_iter()
                .map(|w| NamespaceWindow {
                    name: w.name,
                    window_id: None, // Window ID not assigned by daemon
                    agents: w.agent_names,
                })
                .collect()
        } else {
            // Create default single-window topology
            vec![NamespaceWindow {
                name: "ccb".to_string(),
                window_id: None,
                agents: agent_names.to_vec(),
            }]
        };

        let namespace = ProjectNamespace {
            project_root: project_root.as_str().to_string(),
            project_id: project_id.to_string(),
            tmux_socket_path: tmux_socket_path.to_string(),
            tmux_socket_name: "tmux".to_string(),
            tmux_session_name: tmux_session_name.to_string(),
            agent_names: agent_names.to_vec(),
            windows: namespace_windows,
            agent_panes,
            active_panes,
            namespace_epoch: 1,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let result = StartFlowResult {
            status: "ok".to_string(),
            agent_results,
            actions_taken,
        };

        Ok((result, namespace))
    }

    pub fn to_record(&self, result: &StartFlowResult) -> serde_json::Value {
        serde_json::json!({
            "status": result.status,
            "agent_results": result.agent_results.iter().map(|a| serde_json::json!({
                "agent_name": a.agent_name,
                "status": a.status,
                "reason": a.reason,
                "pane_id": a.pane_id,
            })).collect::<Vec<_>>(),
            "actions_taken": result.actions_taken,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use std::collections::HashSet;

    fn tmp_root() -> (tempfile::TempDir, camino::Utf8PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap().to_path_buf();
        (dir, path)
    }

    #[test]
    fn test_stub_start_flow_creates_panes() {
        let (_dir, root) = tmp_root();
        let service = StartFlowService::with_stub();
        let agents = vec!["alpha".to_string(), "beta".to_string()];
        let registry = AgentRegistry::new();
        let (result, namespace) = service
            .execute(
                &root,
                "pid",
                "/tmp/tmux.sock",
                "session",
                &agents,
                &registry,
                false,
                false,
                None,
                None,
            )
            .unwrap();

        assert_eq!(result.status, "ok");
        assert_eq!(result.agent_results.len(), 2);
        assert_eq!(namespace.agent_panes.len(), 2);
        for agent in &agents {
            assert!(
                namespace.agent_panes.contains_key(agent),
                "missing pane for {agent}"
            );
        }
    }

    #[test]
    fn test_stub_start_flow_reuses_namespace_agent_panes() {
        let (_dir, root) = tmp_root();
        let service = StartFlowService::with_stub();
        let agents = vec!["alpha".to_string(), "beta".to_string()];
        let registry = AgentRegistry::new();
        let mut reused = HashMap::new();
        reused.insert("alpha".to_string(), "%reused-alpha".to_string());
        reused.insert("beta".to_string(), "%reused-beta".to_string());

        let (result, namespace) = service
            .execute(
                &root,
                "pid",
                "/tmp/tmux.sock",
                "session",
                &agents,
                &registry,
                true,
                false,
                Some(&reused),
                None,
            )
            .unwrap();

        assert_eq!(result.status, "ok");
        assert!(result
            .actions_taken
            .iter()
            .any(|a| a.starts_with("use_namespace_topology")));
        assert_eq!(namespace.agent_panes, reused);
        assert!(namespace
            .active_panes
            .contains(&"%reused-alpha".to_string()));
        assert!(namespace.active_panes.contains(&"%reused-beta".to_string()));
    }

    #[test]
    fn test_stub_start_flow_partial_reuse_merges_panes() {
        let (_dir, root) = tmp_root();
        let service = StartFlowService::with_stub();
        let agents = vec!["alpha".to_string(), "beta".to_string()];
        let registry = AgentRegistry::new();
        let mut reused = HashMap::new();
        reused.insert("alpha".to_string(), "%reused-alpha".to_string());

        let (result, namespace) = service
            .execute(
                &root,
                "pid",
                "/tmp/tmux.sock",
                "session",
                &agents,
                &registry,
                false,
                false,
                Some(&reused),
                None,
            )
            .unwrap();

        assert_eq!(result.status, "ok");
        assert!(result
            .actions_taken
            .iter()
            .any(|a| a.starts_with("prepare_tmux_layout:beta")));
        assert_eq!(
            namespace.agent_panes.get("alpha"),
            Some(&"%reused-alpha".to_string())
        );
        assert!(namespace.agent_panes.get("beta").unwrap().starts_with('%'));
        assert!(namespace.active_panes.len() >= 2);
    }

    #[test]
    fn test_stub_start_flow_five_agents_creates_panes() {
        let (_dir, root) = tmp_root();
        let service = StartFlowService::with_stub();
        let agents = vec![
            "a1".to_string(),
            "a2".to_string(),
            "a3".to_string(),
            "a4".to_string(),
            "a5".to_string(),
        ];
        let registry = AgentRegistry::new();
        let (result, namespace) = service
            .execute(
                &root,
                "pid",
                "/tmp/tmux.sock",
                "session",
                &agents,
                &registry,
                false,
                false,
                None,
                None,
            )
            .unwrap();

        assert_eq!(result.status, "ok");
        assert_eq!(result.agent_results.len(), 5);
        assert_eq!(namespace.agent_panes.len(), 5);

        let pane_ids: HashSet<String> = namespace.agent_panes.values().cloned().collect();
        assert_eq!(pane_ids.len(), 5, "each agent should have a unique pane id");
        for (agent, pane) in &namespace.agent_panes {
            assert!(
                pane.starts_with('%'),
                "pane id for {agent} should start with %"
            );
        }
    }
}
