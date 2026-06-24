use std::collections::{BTreeMap, HashMap};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::layout::{parse_layout_spec, LayoutNode};
use crate::models::{
    AgentApiSpec, AgentSpec, MaintenanceHeartbeatConfig, PermissionMode, ProjectConfig,
    ProviderProfileSpec, QueuePolicy, RestoreMode, RuntimeMode, SidebarSpec, SidebarViewSpec,
    ToolWindowSpec, WindowSpec, WorkspaceMode, DEFAULT_MAINTENANCE_HEARTBEAT_ASSESSOR,
    DEFAULT_MAINTENANCE_HEARTBEAT_ESCALATION_POLICY, DEFAULT_MAINTENANCE_HEARTBEAT_INTERVAL_S,
    DEFAULT_MAINTENANCE_HEARTBEAT_MIN_INTERVAL_S, DEFAULT_MAINTENANCE_HEARTBEAT_UNKNOWN_STREAK_CAP,
    SCHEMA_VERSION,
};
use ccbr_storage::paths::PathLayout;

pub const CONFIG_FILENAME: &str = "ccbr.config";
pub const CONFIG_SOURCE_PROJECT: &str = "project_config";
pub const CONFIG_SOURCE_USER: &str = "user_config";
pub const CONFIG_SOURCE_BUILTIN_DEFAULT: &str = "builtin_default";
pub const CONFIG_SOURCE_KINDS: &[&str] = &[
    CONFIG_SOURCE_PROJECT,
    CONFIG_SOURCE_USER,
    CONFIG_SOURCE_BUILTIN_DEFAULT,
];

pub const DEFAULT_CCB_SELF_AGENT: &str = "ccbr_self";
pub const DEFAULT_CCB_SELF_ROLE: &str = "agentroles.ccbr_self";
pub const DEFAULT_AGENT_ORDER: &[&str] = &["agent1", "agent2", "agent3", DEFAULT_CCB_SELF_AGENT];
pub const DEFAULT_DEFAULT_AGENTS: &[&str] = DEFAULT_AGENT_ORDER;

pub const ALLOWED_TOP_LEVEL_KEYS: &[&str] = &[
    "version",
    "default_agents",
    "agents",
    "cmd_enabled",
    "layout",
    "ui",
    "windows",
    "tool_windows",
    "entry_window",
    "maintenance",
];

pub const ALLOWED_AGENT_KEYS: &[&str] = &[
    "provider",
    "target",
    "workspace_mode",
    "workspace_root",
    "workspace_path",
    "workspace_group",
    "provider_command_template",
    "runtime_mode",
    "restore",
    "permission",
    "queue_policy",
    "model",
    "key",
    "url",
    "startup_args",
    "env",
    "api",
    "provider_profile",
    "branch_template",
    "labels",
    "description",
    "role",
    "watch_paths",
];

pub const ALLOWED_PROVIDER_PROFILE_KEYS: &[&str] = &[
    "mode",
    "home",
    "env",
    "inherit_api",
    "inherit_auth",
    "inherit_config",
    "inherit_skills",
    "inherit_commands",
    "inherit_memory",
];

pub const DEFAULT_WINDOW_LAYOUT: &str =
    "agent1:codex, agent2:codex, agent3:claude, ccbr_self:codex";
pub const DEFAULT_TOOL_WINDOW_COMMAND: &str = "ccbr-nvim";

#[derive(Debug, Clone)]
pub struct ConfigLoadResult {
    pub config: ProjectConfig,
    pub source_path: Option<Utf8PathBuf>,
    pub source_kind: String,
    pub used_default: bool,
}

/// Load project config from the .ccbr directory.
pub fn load_project_config(layout: &PathLayout) -> crate::Result<ConfigLoadResult> {
    let config_path = project_config_path(layout);
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let mut config = validate_project_config_text(
            &content,
            Some(&config_path),
            Some(layout.project_root.as_path()),
        )?;
        config.source_path = Some(config_path.to_string());
        return Ok(ConfigLoadResult {
            config,
            source_path: Some(config_path),
            source_kind: CONFIG_SOURCE_PROJECT.into(),
            used_default: false,
        });
    }
    let user_default_path = user_default_config_path();
    if user_default_path.exists() {
        let content = std::fs::read_to_string(&user_default_path)?;
        let mut config = validate_project_config_text(&content, Some(&user_default_path), None)?;
        config.source_path = Some(user_default_path.to_string());
        return Ok(ConfigLoadResult {
            config,
            source_path: Some(user_default_path),
            source_kind: CONFIG_SOURCE_USER.into(),
            used_default: false,
        });
    }
    Ok(ConfigLoadResult {
        config: build_default_project_config(),
        source_path: None,
        source_kind: CONFIG_SOURCE_BUILTIN_DEFAULT.into(),
        used_default: true,
    })
}

pub fn project_config_path(layout: &PathLayout) -> Utf8PathBuf {
    layout.ccbr_dir().join(CONFIG_FILENAME)
}

pub fn user_default_config_path() -> Utf8PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    Utf8PathBuf::from(format!("{}/.ccbr/ccbr.config", home))
}

pub fn ensure_bootstrap_project_config(layout: &PathLayout) -> crate::Result<Utf8PathBuf> {
    let path = project_config_path(layout);
    std::fs::create_dir_all(path.parent().unwrap())?;
    Ok(path)
}

pub fn ensure_default_project_config(layout: &PathLayout) -> crate::Result<Utf8PathBuf> {
    ensure_bootstrap_project_config(layout)
}

pub fn validate_project_config_text(
    text: &str,
    source_path: Option<&Utf8Path>,
    project_root: Option<&Utf8Path>,
) -> crate::Result<ProjectConfig> {
    validate_project_config_text_owned(text, source_path, project_root)
}

fn classify_config_document(text: &str) -> (String, String, Option<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut first_meaningful_kind: Option<&str> = None;
    let mut first_rich_index: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        let body = line
            .split('#')
            .next()
            .unwrap_or("")
            .split("//")
            .next()
            .unwrap_or("")
            .trim();
        if body.is_empty() {
            continue;
        }
        let kind = if body.starts_with('[') || body.contains('=') {
            "rich"
        } else {
            "compact"
        };
        if first_meaningful_kind.is_none() {
            first_meaningful_kind = Some(kind);
        }
        if kind == "rich" {
            first_rich_index = Some(index);
            break;
        }
    }
    match (first_meaningful_kind, first_rich_index) {
        (Some("rich"), _) => ("rich".into(), text.into(), None),
        (Some("compact"), None) => ("compact".into(), text.into(), None),
        (Some("compact"), Some(idx)) => {
            let compact_text = lines[..idx].join("\n");
            let overlay_text = lines[idx..].join("\n");
            ("hybrid".into(), compact_text, Some(overlay_text))
        }
        _ => ("compact".into(), text.into(), None),
    }
}

pub fn validate_project_config_text_owned(
    text: &str,
    source_path: Option<&Utf8Path>,
    project_root: Option<&Utf8Path>,
) -> crate::Result<ProjectConfig> {
    let (kind, primary, overlay) = classify_config_document(text);
    let mut document = match kind.as_str() {
        "rich" => parse_toml_config_document(&primary)?,
        "compact" => parse_compact_config_document(&primary, project_root)?,
        "hybrid" => {
            let mut base = parse_compact_config_document(&primary, project_root)?;
            let overlay_text = overlay
                .ok_or_else(|| crate::AgentError::Config("hybrid config missing overlay".into()))?;
            let overlay_doc = parse_toml_config_document(&overlay_text)?;
            merge_hybrid_overlay(&mut base, &overlay_doc)?;
            base
        }
        _ => {
            return Err(crate::AgentError::Config(format!(
                "unknown config document kind: {kind}"
            )))
        }
    };
    document.source_path = source_path.map(|p| p.to_string());
    document.normalize()?;
    Ok(document)
}

#[derive(Debug, Default, Deserialize)]
struct RawAgentSpec {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    workspace_mode: Option<WorkspaceMode>,
    #[serde(default)]
    workspace_root: Option<String>,
    #[serde(default)]
    workspace_path: Option<String>,
    #[serde(default)]
    workspace_group: Option<String>,
    #[serde(default)]
    provider_command_template: Option<String>,
    #[serde(default)]
    runtime_mode: Option<RuntimeMode>,
    #[serde(default)]
    restore: Option<RestoreMode>,
    #[serde(default)]
    permission: Option<PermissionMode>,
    #[serde(default)]
    queue_policy: Option<QueuePolicy>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    startup_args: Option<Vec<String>>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    api: Option<AgentApiSpec>,
    #[serde(default)]
    provider_profile: Option<ProviderProfileSpec>,
    #[serde(default)]
    branch_template: Option<String>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    watch_paths: Option<Vec<String>>,
}

impl RawAgentSpec {
    fn to_agent_spec(&self, name: &str) -> crate::Result<AgentSpec> {
        let provider = self.provider.clone().ok_or_else(|| {
            crate::AgentError::Config(format!("agents.{name}.provider is required"))
        })?;
        let target = self.target.clone().unwrap_or_else(|| ".".into());
        let workspace_mode = self.workspace_mode.unwrap_or(WorkspaceMode::Inplace);
        let runtime_mode = self.runtime_mode.unwrap_or(RuntimeMode::PaneBacked);
        let restore_default = self.restore.unwrap_or(RestoreMode::Auto);
        let permission_default = self.permission.unwrap_or(PermissionMode::Manual);
        let queue_policy = self.queue_policy.unwrap_or(QueuePolicy::SerialPerAgent);
        let mut api = self.api.clone().unwrap_or_default();
        if api.key.is_none() && api.url.is_none() {
            api.key = self.key.clone();
            api.url = self.url.clone();
        }
        let mut spec = AgentSpec {
            name: name.into(),
            provider,
            target,
            workspace_mode,
            workspace_root: self.workspace_root.clone(),
            runtime_mode,
            restore_default,
            permission_default,
            queue_policy,
            workspace_path: self.workspace_path.clone(),
            workspace_group: self.workspace_group.clone(),
            provider_command_template: self.provider_command_template.clone(),
            model: self.model.clone(),
            startup_args: self.startup_args.clone().unwrap_or_default(),
            env: self.env.clone().unwrap_or_default(),
            api,
            provider_profile: self.provider_profile.clone().unwrap_or_default(),
            branch_template: self.branch_template.clone(),
            labels: self.labels.clone().unwrap_or_default(),
            description: self.description.clone(),
            role: self.role.clone(),
            watch_paths: self.watch_paths.clone().unwrap_or_default(),
        };
        spec.normalize()?;
        Ok(spec)
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawUiTable {
    #[serde(default)]
    sidebar: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMaintenanceTable {
    #[serde(default)]
    heartbeat: Option<RawMaintenanceHeartbeat>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMaintenanceHeartbeat {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    assessor: Option<String>,
    #[serde(default)]
    interval_s: Option<u32>,
    #[serde(default)]
    min_interval_s: Option<u32>,
    #[serde(default)]
    unknown_streak_cap: Option<u32>,
    #[serde(default)]
    escalation_policy: Option<String>,
    #[serde(default)]
    startup_ensure: Option<bool>,
}

impl RawMaintenanceHeartbeat {
    fn to_config(&self) -> MaintenanceHeartbeatConfig {
        MaintenanceHeartbeatConfig {
            enabled: self.enabled.unwrap_or(false),
            assessor: self
                .assessor
                .clone()
                .unwrap_or_else(|| DEFAULT_MAINTENANCE_HEARTBEAT_ASSESSOR.into()),
            interval_s: self
                .interval_s
                .unwrap_or(DEFAULT_MAINTENANCE_HEARTBEAT_INTERVAL_S),
            min_interval_s: self
                .min_interval_s
                .unwrap_or(DEFAULT_MAINTENANCE_HEARTBEAT_MIN_INTERVAL_S),
            unknown_streak_cap: self
                .unknown_streak_cap
                .unwrap_or(DEFAULT_MAINTENANCE_HEARTBEAT_UNKNOWN_STREAK_CAP),
            escalation_policy: self
                .escalation_policy
                .clone()
                .unwrap_or_else(|| DEFAULT_MAINTENANCE_HEARTBEAT_ESCALATION_POLICY.into()),
            startup_ensure: self.startup_ensure.unwrap_or(true),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawProjectConfig {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    default_agents: Option<Vec<String>>,
    #[serde(default)]
    agents: Option<HashMap<String, RawAgentSpec>>,
    #[serde(default)]
    cmd_enabled: Option<bool>,
    #[serde(default)]
    layout: Option<String>,
    #[serde(default)]
    windows: Option<BTreeMap<String, String>>,
    #[serde(default)]
    tool_windows: Option<BTreeMap<String, RawToolWindowSpec>>,
    #[serde(default)]
    entry_window: Option<String>,
    #[serde(default)]
    ui: Option<RawUiTable>,
    #[serde(default)]
    maintenance: Option<RawMaintenanceTable>,
}

#[derive(Debug, Default, Deserialize)]
struct RawToolWindowSpec {
    command: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    show_in_sidebar: Option<bool>,
}

fn parse_toml_config_document(text: &str) -> crate::Result<ProjectConfig> {
    let raw: RawProjectConfig = toml::from_str(text)?;
    raw_to_project_config(raw, false)
}

fn raw_to_project_config(
    raw: RawProjectConfig,
    windows_explicit: bool,
) -> crate::Result<ProjectConfig> {
    let version = raw.version.unwrap_or(SCHEMA_VERSION);
    if version != SCHEMA_VERSION {
        return Err(crate::AgentError::Validation(format!(
            "version must be {SCHEMA_VERSION}"
        )));
    }
    let mut agents: HashMap<String, AgentSpec> = HashMap::new();
    if let Some(raw_agents) = raw.agents {
        for (key, raw_spec) in raw_agents {
            let spec = raw_spec.to_agent_spec(&key)?;
            agents.insert(spec.name.clone(), spec);
        }
    }
    let mut default_agents = raw.default_agents.unwrap_or_default();

    let windows = if let Some(raw_windows) = raw.windows {
        let mut windows: Vec<WindowSpec> = Vec::new();
        for (index, (name, layout_text)) in raw_windows.into_iter().enumerate() {
            let layout = parse_layout_spec(&layout_text)?;
            let mut agent_names = Vec::new();
            for leaf in layout.iter_leaves() {
                if leaf.name.to_lowercase() == "cmd" {
                    return Err(crate::AgentError::Config(
                        "cmd is not supported in windows topology".into(),
                    ));
                }
                if leaf.provider.is_none() {
                    return Err(crate::AgentError::Config(format!(
                        "windows.{name}: agent leaf {:?} must declare a provider",
                        leaf.name
                    )));
                }
                let n = crate::models::normalize_agent_name(&leaf.name)?;
                agent_names.push(n);
            }
            windows.push(WindowSpec {
                name,
                order: index as u32,
                layout_spec: layout.render(),
                agent_names,
            });
        }
        Some(windows)
    } else {
        None
    };

    // Inherit v2 windows-only configs: when `default_agents` is omitted, derive
    // it from the entry window (or the first window) so existing projects that
    // only declare a `[windows]` topology load without rewriting the config.
    if default_agents.is_empty() {
        if let Some(wins) = &windows {
            let chosen = wins
                .iter()
                .find(|w| Some(w.name.as_str()) == raw.entry_window.as_deref())
                .or_else(|| wins.first());
            if let Some(w) = chosen {
                default_agents = w.agent_names.clone();
            }
        }
    }

    let tool_windows = raw.tool_windows.map(|tools| {
        tools
            .into_iter()
            .enumerate()
            .map(|(index, (name, spec))| ToolWindowSpec {
                name,
                order: index as u32,
                command: spec.command,
                label: spec.label,
                show_in_sidebar: spec.show_in_sidebar.unwrap_or(true),
            })
            .collect()
    });

    let sidebar = raw.ui.as_ref().and_then(|ui| ui.sidebar.as_ref()).map(|v| {
        let mut spec: SidebarSpec = v.clone().try_into().unwrap_or_default();
        spec.mode = spec.mode.trim().to_string();
        spec
    });
    let sidebar_view = raw
        .ui
        .as_ref()
        .and_then(|ui| ui.sidebar.as_ref())
        .and_then(|v| {
            v.as_table().cloned().and_then(|map| {
                map.get("view")
                    .and_then(|view| view.clone().try_into().ok())
            })
        });

    let maintenance_heartbeat = raw
        .maintenance
        .map(|m| m.heartbeat.map(|h| h.to_config()).unwrap_or_default());

    Ok(ProjectConfig {
        version,
        default_agents,
        agents,
        cmd_enabled: raw.cmd_enabled.unwrap_or(false),
        layout_spec: raw.layout,
        windows,
        tool_windows,
        entry_window: raw.entry_window,
        sidebar,
        sidebar_view,
        maintenance_heartbeat,
        windows_explicit: Some(windows_explicit),
        topology_signature: None,
        source_path: None,
    })
}

fn parse_compact_config_document(
    text: &str,
    _project_root: Option<&Utf8Path>,
) -> crate::Result<ProjectConfig> {
    let mut layout_text = String::new();
    for line in text.lines() {
        let cleaned = line
            .split('#')
            .next()
            .unwrap_or("")
            .split("//")
            .next()
            .unwrap_or("")
            .trim();
        if !cleaned.is_empty() {
            if !layout_text.is_empty() {
                layout_text.push(' ');
            }
            layout_text.push_str(cleaned);
        }
    }
    if layout_text.is_empty() {
        return Err(crate::AgentError::Config("config is empty".into()));
    }
    let layout = parse_layout_spec(&layout_text)?;
    let mut default_agents = Vec::new();
    let mut agents = HashMap::new();
    let mut cmd_enabled = false;
    for leaf in layout.iter_leaves() {
        let token = leaf.name.trim();
        let normalized = token.to_lowercase();
        if normalized == "cmd" {
            if leaf.provider.is_some() {
                return Err(crate::AgentError::Config(
                    "reserved token 'cmd' cannot declare a provider".into(),
                ));
            }
            if cmd_enabled {
                return Err(crate::AgentError::Config(
                    "compact config cannot define cmd more than once".into(),
                ));
            }
            cmd_enabled = true;
            continue;
        }
        let provider = leaf.provider.clone().ok_or_else(|| {
            crate::AgentError::Config(format!(
                "invalid token {token:?}; expected 'agent_name:provider' or 'cmd'"
            ))
        })?;
        let workspace_mode = if leaf.workspace_mode.as_deref() == Some("worktree") {
            WorkspaceMode::GitWorktree
        } else {
            WorkspaceMode::Inplace
        };
        let spec = AgentSpec {
            name: token.into(),
            provider,
            target: ".".into(),
            workspace_mode,
            ..AgentSpec::default_with_name(token)
        };
        default_agents.push(token.into());
        agents.insert(spec.name.clone(), spec);
    }
    if default_agents.is_empty() {
        return Err(crate::AgentError::Config(
            "compact config must define at least one agent".into(),
        ));
    }
    Ok(ProjectConfig {
        version: SCHEMA_VERSION,
        default_agents,
        agents,
        cmd_enabled,
        layout_spec: Some(layout.render()),
        windows_explicit: Some(false),
        windows: None,
        tool_windows: None,
        entry_window: None,
        sidebar: None,
        sidebar_view: None,
        maintenance_heartbeat: None,
        ..ProjectConfig::default()
    })
}

impl AgentSpec {
    pub fn default_with_name(name: &str) -> Self {
        Self {
            name: name.into(),
            provider: String::new(),
            target: ".".into(),
            workspace_mode: WorkspaceMode::Inplace,
            workspace_root: None,
            runtime_mode: RuntimeMode::PaneBacked,
            restore_default: RestoreMode::Auto,
            permission_default: PermissionMode::Manual,
            queue_policy: QueuePolicy::SerialPerAgent,
            workspace_path: None,
            workspace_group: None,
            provider_command_template: None,
            model: None,
            startup_args: Vec::new(),
            env: HashMap::new(),
            api: AgentApiSpec::default(),
            provider_profile: ProviderProfileSpec::default(),
            branch_template: None,
            labels: Vec::new(),
            description: None,
            role: None,
            watch_paths: Vec::new(),
        }
    }
}

fn merge_hybrid_overlay(base: &mut ProjectConfig, overlay: &ProjectConfig) -> crate::Result<()> {
    for (name, overlay_spec) in &overlay.agents {
        if !base.agents.contains_key(name) {
            return Err(crate::AgentError::Config(format!(
                "hybrid overlay cannot define agent {name:?} outside the compact layout"
            )));
        }
        if overlay_spec.provider != base.agents[name].provider {
            return Err(crate::AgentError::Config(format!(
                "hybrid overlay cannot redefine provider for agents.{name}"
            )));
        }
        if overlay_spec.workspace_mode != base.agents[name].workspace_mode {
            return Err(crate::AgentError::Config(format!(
                "hybrid overlay cannot redefine workspace_mode for agents.{name}"
            )));
        }
        base.agents.insert(name.clone(), overlay_spec.clone());
    }
    if overlay.maintenance_heartbeat.is_some() {
        base.maintenance_heartbeat = overlay.maintenance_heartbeat.clone();
    }
    Ok(())
}

pub fn build_default_project_config() -> ProjectConfig {
    let default_agents: Vec<String> = DEFAULT_DEFAULT_AGENTS.iter().map(|s| (*s).into()).collect();
    let mut agents = HashMap::new();
    agents.insert(
        "agent1".into(),
        build_default_agent_spec("agent1", "codex", None),
    );
    agents.insert(
        "agent2".into(),
        build_default_agent_spec("agent2", "codex", None),
    );
    agents.insert(
        "agent3".into(),
        build_default_agent_spec("agent3", "claude", None),
    );
    agents.insert(
        "ccbr_self".into(),
        build_default_agent_spec("ccbr_self", "codex", Some(DEFAULT_CCB_SELF_ROLE)),
    );
    let windows = vec![WindowSpec {
        name: "main".into(),
        order: 0,
        layout_spec: DEFAULT_WINDOW_LAYOUT.into(),
        agent_names: default_agents.clone(),
    }];
    let tool_windows = vec![ToolWindowSpec {
        name: "neovim".into(),
        order: 0,
        command: DEFAULT_TOOL_WINDOW_COMMAND.into(),
        label: Some("neovim".into()),
        show_in_sidebar: true,
    }];
    let mut config = ProjectConfig {
        version: SCHEMA_VERSION,
        default_agents,
        agents,
        cmd_enabled: false,
        layout_spec: None,
        windows: Some(windows),
        tool_windows: Some(tool_windows),
        entry_window: Some("main".into()),
        sidebar: Some(SidebarSpec::default()),
        sidebar_view: Some(SidebarViewSpec::default()),
        maintenance_heartbeat: Some(MaintenanceHeartbeatConfig::default()),
        windows_explicit: Some(true),
        topology_signature: None,
        source_path: None,
    };
    config.normalize().expect("default config is valid");
    config
}

pub fn build_default_agent_spec(name: &str, provider: &str, role: Option<&str>) -> AgentSpec {
    AgentSpec {
        name: name.into(),
        provider: provider.into(),
        target: ".".into(),
        workspace_mode: WorkspaceMode::Inplace,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
        restore_default: RestoreMode::Auto,
        permission_default: PermissionMode::Manual,
        queue_policy: QueuePolicy::SerialPerAgent,
        workspace_path: None,
        workspace_group: None,
        provider_command_template: None,
        model: None,
        startup_args: Vec::new(),
        env: HashMap::new(),
        api: AgentApiSpec::default(),
        provider_profile: ProviderProfileSpec::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: role.map(|s| s.into()),
        watch_paths: Vec::new(),
    }
}

pub fn render_default_project_config_text() -> String {
    render_project_config_text(&build_default_project_config())
}

pub fn render_project_config_text(config: &ProjectConfig) -> String {
    if config.windows_explicit.unwrap_or(false) {
        return render_windows_config_text(config);
    }
    if can_render_compact(config) {
        return format!("{}\n", config.layout_spec.as_deref().unwrap_or(""));
    }
    let layout = render_hybrid_layout(config);
    let overlay = build_hybrid_overlay_payload(config);
    if overlay.agents.is_empty() && overlay.maintenance_heartbeat.is_none() {
        return format!("{layout}\n");
    }
    format!("{layout}\n\n{}", render_toml_document(&overlay))
}

fn render_windows_config_text(config: &ProjectConfig) -> String {
    let mut payload = serde_json::json!({
        "version": config.version,
        "entry_window": config.entry_window.as_deref().unwrap_or("main"),
        "windows": config.windows.as_ref().unwrap().iter().map(|w| (w.name.clone(), serde_json::Value::String(w.layout_spec.clone()))).collect::<serde_json::Map<String, serde_json::Value>>(),
    });
    if let Some(tools) = &config.tool_windows {
        let tool_payload: serde_json::Map<String, serde_json::Value> = tools
            .iter()
            .map(|t| {
                let mut obj = serde_json::json!({
                    "command": t.command,
                });
                let obj_map = obj.as_object_mut().unwrap();
                if t.label.as_deref() != Some(&t.name) {
                    obj_map.insert("label".into(), t.label.clone().into());
                }
                if !t.show_in_sidebar {
                    obj_map.insert("show_in_sidebar".into(), false.into());
                }
                (t.name.clone(), obj)
            })
            .collect();
        if !tool_payload.is_empty() {
            payload["tool_windows"] = tool_payload.into();
        }
    }
    if let Some(sidebar) = &config.sidebar {
        let mut sidebar_payload = serde_json::Map::new();
        if sidebar.mode != crate::models::SIDEBAR_MODE_EVERY_WINDOW {
            sidebar_payload.insert("mode".into(), sidebar.mode.clone().into());
        }
        if sidebar.width.as_string() != "15%" {
            sidebar_payload.insert("width".into(), sidebar.width.as_string().into());
        }
        if sidebar.bottom_height != 20 {
            sidebar_payload.insert("bottom_height".into(), sidebar.bottom_height.into());
        }
        if let Some(sidebar_view) = &config.sidebar_view {
            sidebar_payload.insert(
                "view".into(),
                serde_json::json!({
                    "agents_height": sidebar_view.agents_height.as_string(),
                    "comms_height": sidebar_view.comms_height.as_string(),
                    "tips_height": sidebar_view.tips_height.as_string(),
                }),
            );
        }
        if !sidebar_payload.is_empty() {
            let mut ui = serde_json::Map::new();
            ui.insert("sidebar".into(), sidebar_payload.into());
            payload["ui"] = ui.into();
        }
    }
    let agent_payload = build_hybrid_overlay_payload(config);
    if !agent_payload.agents.is_empty() {
        let mut agents_map = serde_json::Map::new();
        for (name, spec) in &agent_payload.agents {
            agents_map.insert(name.clone(), spec.to_record());
        }
        payload["agents"] = agents_map.into();
    }
    render_toml_document_from_json(&payload)
}

#[derive(Debug, Default)]
struct OverlayPayload {
    agents: HashMap<String, AgentSpec>,
    maintenance_heartbeat: Option<MaintenanceHeartbeatConfig>,
}

fn build_hybrid_overlay_payload(config: &ProjectConfig) -> OverlayPayload {
    let mut payload = OverlayPayload::default();
    let compact_defaults = compact_agent_defaults_by_name(config);
    let ordered_names: Vec<String> = config
        .default_agents
        .iter()
        .cloned()
        .chain(
            config
                .agents
                .keys()
                .filter(|k| !config.default_agents.contains(k))
                .cloned(),
        )
        .collect();
    for name in ordered_names {
        let spec = match config.agents.get(&name) {
            Some(s) => s,
            None => continue,
        };
        let (compact_provider, compact_workspace_mode) = match compact_defaults.get(&name) {
            Some(d) => (d.provider.clone(), d.workspace_mode.clone()),
            None => continue,
        };
        if let Some(overlay) =
            agent_spec_to_hybrid_overlay_dict(spec, &compact_provider, &compact_workspace_mode)
        {
            payload.agents.insert(name, overlay);
        }
    }
    payload
}

fn agent_spec_to_hybrid_overlay_dict(
    spec: &AgentSpec,
    compact_provider: &str,
    compact_workspace_mode: &str,
) -> Option<AgentSpec> {
    let full = agent_spec_to_config_dict(spec);
    let defaults: HashMap<&str, serde_json::Value> = [
        ("provider", compact_provider.into()),
        ("target", ".".into()),
        ("workspace_mode", compact_workspace_mode.into()),
        ("runtime_mode", "pane-backed".into()),
        ("restore", "auto".into()),
        ("permission", "manual".into()),
        ("queue_policy", "serial-per-agent".into()),
    ]
    .into_iter()
    .collect();
    let mut overlay = serde_json::Map::new();
    for (key, value) in full.as_object().unwrap() {
        if let Some(expected) = defaults.get(key.as_str()) {
            if value == expected {
                continue;
            }
        }
        overlay.insert(key.clone(), value.clone());
    }
    if overlay.is_empty() {
        return None;
    }
    let mut cloned = spec.clone();
    cloned.provider = overlay
        .get("provider")
        .and_then(|v| v.as_str().map(|s| s.into()))
        .unwrap_or_else(|| compact_provider.into());
    cloned.target = overlay
        .get("target")
        .and_then(|v| v.as_str().map(|s| s.into()))
        .unwrap_or_else(|| ".".into());
    cloned.workspace_mode = overlay
        .get("workspace_mode")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "git-worktree" => WorkspaceMode::GitWorktree,
            "copy" => WorkspaceMode::Copy,
            _ => WorkspaceMode::Inplace,
        })
        .unwrap_or(WorkspaceMode::Inplace);
    Some(cloned)
}

fn agent_spec_to_config_dict(spec: &AgentSpec) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "provider": spec.provider,
        "target": spec.target,
        "workspace_mode": spec.workspace_mode,
        "runtime_mode": spec.runtime_mode,
        "restore": spec.restore_default,
        "permission": spec.permission_default,
        "queue_policy": spec.queue_policy,
    });
    let obj = payload.as_object_mut().unwrap();
    if let Some(root) = &spec.workspace_root {
        obj.insert("workspace_root".into(), root.clone().into());
    }
    if let Some(path) = &spec.workspace_path {
        obj.insert("workspace_path".into(), path.clone().into());
    }
    if let Some(group) = &spec.workspace_group {
        obj.insert("workspace_group".into(), group.clone().into());
    }
    if let Some(tpl) = &spec.provider_command_template {
        obj.insert("provider_command_template".into(), tpl.clone().into());
    }
    if let Some(model) = &spec.model {
        obj.insert("model".into(), model.clone().into());
    }
    if !spec.startup_args.is_empty() {
        obj.insert("startup_args".into(), spec.startup_args.clone().into());
    }
    if !spec.env.is_empty() {
        obj.insert("env".into(), serde_json::json!(spec.env));
    }
    if spec.api != AgentApiSpec::default() {
        if let Some(key) = &spec.api.key {
            obj.insert("key".into(), key.clone().into());
        }
        if let Some(url) = &spec.api.url {
            obj.insert("url".into(), url.clone().into());
        }
    }
    let profile_payload = provider_profile_config_dict(spec);
    if let Some(p) = profile_payload {
        obj.insert("provider_profile".into(), p);
    }
    if let Some(bt) = &spec.branch_template {
        obj.insert("branch_template".into(), bt.clone().into());
    }
    if !spec.labels.is_empty() {
        obj.insert("labels".into(), spec.labels.clone().into());
    }
    if let Some(desc) = &spec.description {
        obj.insert("description".into(), desc.clone().into());
    }
    if let Some(role) = &spec.role {
        obj.insert("role".into(), role.clone().into());
    }
    if !spec.watch_paths.is_empty() {
        obj.insert("watch_paths".into(), spec.watch_paths.clone().into());
    }
    payload
}

fn provider_profile_config_dict(spec: &AgentSpec) -> Option<serde_json::Value> {
    let profile = &spec.provider_profile;
    let default_profile = ProviderProfileSpec::default();
    if spec.api == AgentApiSpec::default() {
        if profile == &default_profile {
            return None;
        }
        return Some(profile.to_record());
    }
    let api_keys = ccbr_provider_profiles::materializer::provider_api_env_keys(&spec.provider);
    let filtered_env: HashMap<String, String> = profile
        .env
        .iter()
        .filter(|(k, _)| !api_keys.contains(k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut payload = serde_json::Map::new();
    if profile.mode != default_profile.mode {
        payload.insert("mode".into(), profile.mode.clone().into());
    }
    if profile.home.is_some() {
        payload.insert("home".into(), profile.home.clone().into());
    }
    if !filtered_env.is_empty() {
        payload.insert("env".into(), serde_json::json!(filtered_env));
    }
    if profile.inherit_auth != default_profile.inherit_auth {
        payload.insert("inherit_auth".into(), profile.inherit_auth.into());
    }
    if profile.inherit_config != default_profile.inherit_config {
        payload.insert("inherit_config".into(), profile.inherit_config.into());
    }
    if profile.inherit_skills != default_profile.inherit_skills {
        payload.insert("inherit_skills".into(), profile.inherit_skills.into());
    }
    if profile.inherit_commands != default_profile.inherit_commands {
        payload.insert("inherit_commands".into(), profile.inherit_commands.into());
    }
    if profile.inherit_memory != default_profile.inherit_memory {
        payload.insert("inherit_memory".into(), profile.inherit_memory.into());
    }
    if payload.is_empty() {
        None
    } else {
        Some(payload.into())
    }
}

fn compact_agent_defaults_by_name(config: &ProjectConfig) -> HashMap<String, CompactDefaults> {
    let layout_text = config.layout_spec.clone().unwrap_or_default();
    let layout = parse_layout_spec(&layout_text).unwrap_or_else(|_| LayoutNode::leaf(""));
    let mut defaults = HashMap::new();
    for leaf in layout.iter_leaves() {
        let name = leaf.name.to_lowercase();
        if name == "cmd" {
            continue;
        }
        let n = crate::models::normalize_agent_name(&leaf.name)
            .unwrap_or_else(|_| leaf.name.to_lowercase());
        defaults.insert(
            n,
            CompactDefaults {
                provider: leaf.provider.clone().unwrap_or_default(),
                workspace_mode: if leaf.workspace_mode.as_deref() == Some("worktree") {
                    "git-worktree".into()
                } else {
                    "inplace".into()
                },
            },
        );
    }
    defaults
}

#[derive(Debug, Clone)]
struct CompactDefaults {
    provider: String,
    workspace_mode: String,
}

fn render_hybrid_layout(config: &ProjectConfig) -> String {
    let layout_text = config.layout_spec.clone().unwrap_or_default();
    let layout = parse_layout_spec(&layout_text).unwrap_or_else(|_| LayoutNode::leaf(""));
    annotate_layout_with_agent_specs(&layout, config).render()
}

fn annotate_layout_with_agent_specs(node: &LayoutNode, config: &ProjectConfig) -> LayoutNode {
    match node {
        LayoutNode::Leaf { leaf } => {
            let name = leaf.name.trim();
            if name.to_lowercase() == "cmd" {
                let mut cmd_leaf = leaf.clone();
                cmd_leaf.provider = None;
                return LayoutNode::Leaf { leaf: cmd_leaf };
            }
            let normalized =
                crate::models::normalize_agent_name(name).unwrap_or_else(|_| name.to_lowercase());
            let spec = config.agents.get(&normalized);
            let mut new_leaf = leaf.clone();
            if let Some(spec) = spec {
                new_leaf.provider = Some(spec.provider.clone());
                new_leaf.workspace_mode = if spec.workspace_mode == WorkspaceMode::GitWorktree {
                    Some("worktree".into())
                } else {
                    None
                };
            }
            LayoutNode::Leaf { leaf: new_leaf }
        }
        LayoutNode::Horizontal { left, right } => LayoutNode::Horizontal {
            left: Box::new(annotate_layout_with_agent_specs(left, config)),
            right: Box::new(annotate_layout_with_agent_specs(right, config)),
        },
        LayoutNode::Vertical { left, right } => LayoutNode::Vertical {
            left: Box::new(annotate_layout_with_agent_specs(left, config)),
            right: Box::new(annotate_layout_with_agent_specs(right, config)),
        },
    }
}

fn can_render_compact(config: &ProjectConfig) -> bool {
    if !config.cmd_enabled {
        return false;
    }
    config.default_agents.iter().all(|name| {
        config
            .agents
            .get(name)
            .map(is_compact_agent_compatible)
            .unwrap_or(false)
    })
}

fn is_compact_agent_compatible(spec: &AgentSpec) -> bool {
    core_agent_defaults_match(spec)
        && spec.api == AgentApiSpec::default()
        && spec.provider_profile == ProviderProfileSpec::default()
        && spec.branch_template.is_none()
        && spec.labels.is_empty()
        && spec.description.is_none()
        && spec.watch_paths.is_empty()
}

fn core_agent_defaults_match(spec: &AgentSpec) -> bool {
    spec.target == "."
        && (spec.workspace_mode == WorkspaceMode::Inplace
            || spec.workspace_mode == WorkspaceMode::GitWorktree)
        && spec.workspace_root.is_none()
        && spec.runtime_mode == RuntimeMode::PaneBacked
        && spec.restore_default == RestoreMode::Auto
        && spec.permission_default == PermissionMode::Manual
        && spec.queue_policy == QueuePolicy::SerialPerAgent
        && spec.model.is_none()
        && spec.startup_args.is_empty()
        && spec.env.is_empty()
}

fn render_toml_document(payload: &OverlayPayload) -> String {
    let mut lines: Vec<String> = Vec::new();
    if !payload.agents.is_empty() {
        for (name, spec) in &payload.agents {
            let dict = agent_spec_to_config_dict(spec);
            lines.push(format!("[agents.{}]", name));
            render_toml_object_lines(&mut lines, dict.as_object().unwrap());
            lines.push(String::new());
        }
    }
    lines.join("\n").trim_end().to_string() + "\n"
}

fn render_toml_document_from_json(payload: &serde_json::Value) -> String {
    let mut lines: Vec<String> = Vec::new();
    render_toml_value_lines(&mut lines, &[], payload, false, false);
    lines.join("\n").trim_end().to_string() + "\n"
}

fn render_toml_value_lines(
    lines: &mut Vec<String>,
    path: &[String],
    value: &serde_json::Value,
    emit_header: bool,
    is_array: bool,
) {
    match value {
        serde_json::Value::Object(map) => {
            let mut scalars: Vec<(&String, &serde_json::Value)> = Vec::new();
            let mut tables: Vec<(&String, &serde_json::Value)> = Vec::new();
            let mut arrays: Vec<(&String, &serde_json::Value)> = Vec::new();
            for (k, v) in map {
                match v {
                    serde_json::Value::Object(_) => tables.push((k, v)),
                    serde_json::Value::Array(arr)
                        if !arr.is_empty() && arr.iter().all(|x| x.is_object()) =>
                    {
                        arrays.push((k, v))
                    }
                    _ => scalars.push((k, v)),
                }
            }
            if emit_header
                && (is_array || !scalars.is_empty() || tables.is_empty() && arrays.is_empty())
            {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                let header = if is_array {
                    format!("[[{}]]", render_toml_path(path))
                } else {
                    format!("[{}]", render_toml_path(path))
                };
                lines.push(header);
            }
            for (k, v) in scalars {
                lines.push(format!("{} = {}", render_toml_key(k), render_toml_value(v)));
            }
            for (k, v) in tables {
                let mut new_path = path.to_vec();
                new_path.push(k.clone());
                render_toml_value_lines(lines, &new_path, v, true, false);
            }
            for (k, v) in arrays {
                let mut new_path = path.to_vec();
                new_path.push(k.clone());
                if let serde_json::Value::Array(arr) = v {
                    for item in arr {
                        render_toml_value_lines(lines, &new_path, item, true, true);
                    }
                }
            }
        }
        _ => {
            if emit_header {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.push(format!(
                    "{} = {}",
                    render_toml_path(path),
                    render_toml_value(value)
                ));
            }
        }
    }
}

fn render_toml_path(path: &[String]) -> String {
    path.iter()
        .map(|p| render_toml_key(p))
        .collect::<Vec<_>>()
        .join(".")
}

fn render_toml_key(key: &str) -> String {
    if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        key.to_string()
    } else {
        serde_json::to_string(key).unwrap_or_else(|_| format!("\"{}\"", key))
    }
}

fn render_toml_object_lines(
    lines: &mut Vec<String>,
    obj: &serde_json::Map<String, serde_json::Value>,
) {
    for (k, v) in obj {
        lines.push(format!("{} = {}", render_toml_key(k), render_toml_value(v)));
    }
}

fn render_toml_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Bool(b) => (if *b { "true" } else { "false" }).into(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(render_toml_value).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(map) => {
            let pairs: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{} = {}", render_toml_key(k), render_toml_value(v)))
                .collect();
            format!("{{ {} }}", pairs.join(", "))
        }
        serde_json::Value::Null => "null".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_missing_config_returns_default() {
        let dir = TempDir::new().unwrap();
        let p = camino::Utf8Path::from_path(dir.path()).unwrap();
        let layout = PathLayout::new(p);
        let result = load_project_config(&layout).unwrap();
        assert_eq!(result.config.version, SCHEMA_VERSION);
        assert_eq!(result.source_kind, CONFIG_SOURCE_BUILTIN_DEFAULT);
        assert!(result.used_default);
    }

    #[test]
    fn test_config_path() {
        let layout = PathLayout::new("/project");
        let path = project_config_path(&layout);
        assert!(path.as_str().ends_with(".ccbr/ccbr.config"));
    }

    #[test]
    fn test_build_default_project_config() {
        let config = build_default_project_config();
        assert_eq!(config.version, SCHEMA_VERSION);
        assert!(!config.agents.is_empty());
    }

    #[test]
    fn test_render_default_project_config_text() {
        let text = render_default_project_config_text();
        assert!(text.contains("version = 2"));
    }
}
