use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

pub use ccb_provider_profiles::ProviderProfileSpec;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::layout::{build_balanced_layout, parse_layout_spec, prune_layout, LayoutNode};
use crate::roles::canonical_role_id;

pub const SCHEMA_VERSION: u32 = 2;

fn agent_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]{0,31}$").unwrap())
}

pub const RESERVED_AGENT_NAMES: &[&str] = &[
    "all", "from", "user", "system", "ask", "cancel", "clear", "pend", "ping", "watch", "kill",
    "ps", "logs", "doctor", "config", "cmd", "version", "update", "help",
];

pub fn normalize_agent_name(name: &str) -> crate::Result<String> {
    let value = name.trim();
    if value.is_empty() {
        return Err(crate::AgentError::Validation(
            "agent name cannot be empty".into(),
        ));
    }
    if !agent_name_re().is_match(value) {
        return Err(crate::AgentError::Validation(
            "agent name must match ^[a-zA-Z][a-zA-Z0-9_-]{0,31}$".into(),
        ));
    }
    let normalized = value.to_lowercase();
    if RESERVED_AGENT_NAMES.contains(&normalized.as_str()) {
        return Err(crate::AgentError::Validation(format!(
            "agent name {:?} is reserved",
            normalized
        )));
    }
    Ok(normalized)
}

pub fn validate_agent_name(name: &str) -> crate::Result<String> {
    normalize_agent_name(name)
}

pub fn normalize_runtime_mode(value: &str) -> crate::Result<RuntimeMode> {
    let raw = value.trim().to_lowercase();
    match raw.as_str() {
        "pane" | "pane-backed" => Ok(RuntimeMode::PaneBacked),
        "pty" | "pty-backed" => Ok(RuntimeMode::PtyBacked),
        "headless" => Ok(RuntimeMode::Headless),
        _ => Err(crate::AgentError::Validation(
            "runtime_mode must be one of: pane-backed, pty-backed, headless".into(),
        )),
    }
}

pub fn normalize_runtime_binding_source(value: &str) -> crate::Result<RuntimeBindingSource> {
    let raw = value.trim().to_lowercase();
    match raw.as_str() {
        "provider" | "provider-session" => Ok(RuntimeBindingSource::ProviderSession),
        "external" | "external-attach" => Ok(RuntimeBindingSource::ExternalAttach),
        _ => Err(crate::AgentError::Validation(
            "runtime binding source must be one of: provider-session, external-attach".into(),
        )),
    }
}

// --- Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum WorkspaceMode {
    GitWorktree,
    Copy,
    #[default]
    Inplace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum RuntimeMode {
    #[default]
    PaneBacked,
    PtyBacked,
    Headless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum RuntimeBindingSource {
    #[default]
    ProviderSession,
    ExternalAttach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum RestoreMode {
    #[default]
    Fresh,
    Provider,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PermissionMode {
    #[default]
    Manual,
    Auto,
    Readonly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum QueuePolicy {
    #[default]
    SerialPerAgent,
    RejectWhenBusy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AgentState {
    #[default]
    Starting,
    Idle,
    Busy,
    Stopping,
    Stopped,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RestoreStatus {
    #[default]
    Fresh,
    Provider,
    Checkpoint,
    Failed,
}

// --- Agent API spec ---

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentApiSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl AgentApiSpec {
    pub fn normalize(&mut self) {
        self.key = self
            .key
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self.url = self
            .url
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "key": self.key,
            "url": self.url,
        })
    }
}

// --- Maintenance heartbeat config ---

pub const DEFAULT_MAINTENANCE_HEARTBEAT_ASSESSOR: &str = "ccb_self";
pub const DEFAULT_MAINTENANCE_HEARTBEAT_INTERVAL_S: u32 = 3600;
pub const DEFAULT_MAINTENANCE_HEARTBEAT_MIN_INTERVAL_S: u32 = 300;
pub const DEFAULT_MAINTENANCE_HEARTBEAT_UNKNOWN_STREAK_CAP: u32 = 3;
pub const DEFAULT_MAINTENANCE_HEARTBEAT_ESCALATION_POLICY: &str = "report_only";
pub const MAINTENANCE_HEARTBEAT_ESCALATION_POLICIES: &[&str] = &["ask_user", "report_only"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceHeartbeatConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_maintenance_heartbeat_assessor")]
    pub assessor: String,
    #[serde(default = "default_maintenance_heartbeat_interval_s")]
    pub interval_s: u32,
    #[serde(default = "default_maintenance_heartbeat_min_interval_s")]
    pub min_interval_s: u32,
    #[serde(default = "default_maintenance_heartbeat_unknown_streak_cap")]
    pub unknown_streak_cap: u32,
    #[serde(default = "default_maintenance_heartbeat_escalation_policy")]
    pub escalation_policy: String,
    #[serde(default = "default_maintenance_heartbeat_startup_ensure")]
    pub startup_ensure: bool,
}

fn default_maintenance_heartbeat_assessor() -> String {
    DEFAULT_MAINTENANCE_HEARTBEAT_ASSESSOR.into()
}
fn default_maintenance_heartbeat_interval_s() -> u32 {
    DEFAULT_MAINTENANCE_HEARTBEAT_INTERVAL_S
}
fn default_maintenance_heartbeat_min_interval_s() -> u32 {
    DEFAULT_MAINTENANCE_HEARTBEAT_MIN_INTERVAL_S
}
fn default_maintenance_heartbeat_unknown_streak_cap() -> u32 {
    DEFAULT_MAINTENANCE_HEARTBEAT_UNKNOWN_STREAK_CAP
}
fn default_maintenance_heartbeat_escalation_policy() -> String {
    DEFAULT_MAINTENANCE_HEARTBEAT_ESCALATION_POLICY.into()
}
fn default_maintenance_heartbeat_startup_ensure() -> bool {
    true
}

impl Default for MaintenanceHeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            assessor: DEFAULT_MAINTENANCE_HEARTBEAT_ASSESSOR.into(),
            interval_s: DEFAULT_MAINTENANCE_HEARTBEAT_INTERVAL_S,
            min_interval_s: DEFAULT_MAINTENANCE_HEARTBEAT_MIN_INTERVAL_S,
            unknown_streak_cap: DEFAULT_MAINTENANCE_HEARTBEAT_UNKNOWN_STREAK_CAP,
            escalation_policy: DEFAULT_MAINTENANCE_HEARTBEAT_ESCALATION_POLICY.into(),
            startup_ensure: true,
        }
    }
}

impl MaintenanceHeartbeatConfig {
    pub fn validate(&self) -> crate::Result<()> {
        let assessor = normalize_agent_name(&self.assessor).map_err(|e| {
            crate::AgentError::Validation(format!("maintenance.heartbeat.assessor invalid: {e}"))
        })?;
        if self.interval_s == 0 {
            return Err(crate::AgentError::Validation(
                "maintenance.heartbeat.interval_s must be a positive integer".into(),
            ));
        }
        if self.min_interval_s == 0 {
            return Err(crate::AgentError::Validation(
                "maintenance.heartbeat.min_interval_s must be a positive integer".into(),
            ));
        }
        if self.min_interval_s > self.interval_s {
            return Err(crate::AgentError::Validation(
                "maintenance.heartbeat.min_interval_s cannot exceed interval_s".into(),
            ));
        }
        if self.unknown_streak_cap == 0 {
            return Err(crate::AgentError::Validation(
                "maintenance.heartbeat.unknown_streak_cap must be a positive integer".into(),
            ));
        }
        let policy = self.escalation_policy.trim().to_lowercase();
        if !MAINTENANCE_HEARTBEAT_ESCALATION_POLICIES.contains(&policy.as_str()) {
            return Err(crate::AgentError::Validation(format!(
                "maintenance.heartbeat.escalation_policy must be one of: {}",
                MAINTENANCE_HEARTBEAT_ESCALATION_POLICIES.join(", ")
            )));
        }
        // Use normalized values via clone to avoid mutation complexity in validation.
        let _ = assessor;
        let _ = policy;
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.enabled,
            "assessor": self.assessor,
            "interval_s": self.interval_s,
            "min_interval_s": self.min_interval_s,
            "unknown_streak_cap": self.unknown_streak_cap,
            "escalation_policy": self.escalation_policy,
            "startup_ensure": self.startup_ensure,
        })
    }
}

// --- Sidebar dimensions ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SidebarDimension {
    Percent(String),
    Pixels(u32),
}

impl SidebarDimension {
    pub fn parse(value: &str, field_name: &str, allow_full: bool) -> crate::Result<Self> {
        let text = value.trim();
        if text.is_empty() {
            return Err(crate::AgentError::Validation(format!(
                "{field_name} must be a positive integer or percentage string"
            )));
        }
        if let Some(number) = text.strip_suffix('%') {
            let number = number.trim();
            if let Ok(n) = number.parse::<u32>() {
                if n == 0 || (!allow_full && n >= 100) {
                    return Err(crate::AgentError::Validation(format!(
                        "{field_name} percentage must be {}",
                        if allow_full {
                            "positive"
                        } else {
                            "between 1% and 99%"
                        }
                    )));
                }
                return Ok(SidebarDimension::Percent(format!("{n}%")));
            }
        }
        if let Ok(n) = text.parse::<u32>() {
            if n == 0 {
                return Err(crate::AgentError::Validation(format!(
                    "{field_name} must be positive"
                )));
            }
            return Ok(SidebarDimension::Pixels(n));
        }
        Err(crate::AgentError::Validation(format!(
            "{field_name} must be a positive integer or percentage string"
        )))
    }

    pub fn as_string(&self) -> String {
        match self {
            SidebarDimension::Percent(s) => s.clone(),
            SidebarDimension::Pixels(n) => n.to_string(),
        }
    }
}

impl Default for SidebarDimension {
    fn default() -> Self {
        SidebarDimension::Percent("15%".into())
    }
}

// --- Sidebar specs ---

pub const SIDEBAR_MODE_EVERY_WINDOW: &str = "every_window";
pub const SIDEBAR_MODE_OFF: &str = "off";

pub const DEFAULT_SIDEBAR_VIEW_TIPS: &[&str] = &[
    "C-b d  detach",
    "C-b h/j/k/l pane",
    "C-b H/J/K/L resize",
    "C-b o  next pane",
    "C-b z  zoom",
    "C-b w  tree",
    "C-b n/p next/prev",
    "C-b 0-9 jump win",
    "C-b [  copy mode",
    "copy: PgUp/PgDn",
    "copy: v select",
    "copy: y yank",
    "copy: q exit",
    "C-b ]  paste",
    "C-b c  new win",
    "C-b ,  rename",
    "C-b ?  keys",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarSpec {
    #[serde(default = "default_sidebar_mode")]
    pub mode: String,
    #[serde(default)]
    pub width: SidebarDimension,
    #[serde(default = "default_sidebar_bottom_height")]
    pub bottom_height: u32,
}

fn default_sidebar_mode() -> String {
    SIDEBAR_MODE_EVERY_WINDOW.into()
}
fn default_sidebar_bottom_height() -> u32 {
    20
}

impl Default for SidebarSpec {
    fn default() -> Self {
        Self {
            mode: SIDEBAR_MODE_EVERY_WINDOW.into(),
            width: SidebarDimension::Percent("15%".into()),
            bottom_height: 20,
        }
    }
}

impl SidebarSpec {
    pub fn validate(&self) -> crate::Result<()> {
        let mode = self.mode.trim();
        if mode != SIDEBAR_MODE_EVERY_WINDOW && mode != SIDEBAR_MODE_OFF {
            return Err(crate::AgentError::Validation(
                "ui.sidebar.mode must be every_window or off".into(),
            ));
        }
        match &self.width {
            SidebarDimension::Pixels(n) if *n > 0 => {}
            SidebarDimension::Percent(s) => {
                let num = s.trim_end_matches('%').parse::<u32>().unwrap_or(0);
                if num == 0 {
                    return Err(crate::AgentError::Validation(
                        "ui.sidebar.width percentage must be positive".into(),
                    ));
                }
            }
            _ => {
                return Err(crate::AgentError::Validation(
                    "ui.sidebar.width must be positive".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": self.mode,
            "width": self.width.as_string(),
            "bottom_height": self.bottom_height,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarViewSpec {
    #[serde(default)]
    pub agents_height: SidebarDimension,
    #[serde(default = "default_comms_height")]
    pub comms_height: SidebarDimension,
    #[serde(default = "default_tips_height")]
    pub tips_height: SidebarDimension,
    #[serde(default = "default_comms_limit")]
    pub comms_limit: u32,
    #[serde(default = "default_true")]
    pub comms_compact: bool,
    #[serde(default = "default_true")]
    pub tips_enabled: bool,
    #[serde(default = "default_tips")]
    pub tips: Vec<String>,
}

fn default_comms_height() -> SidebarDimension {
    SidebarDimension::Percent("15%".into())
}
fn default_tips_height() -> SidebarDimension {
    SidebarDimension::Percent("35%".into())
}
fn default_comms_limit() -> u32 {
    5
}
fn default_true() -> bool {
    true
}
fn default_tips() -> Vec<String> {
    DEFAULT_SIDEBAR_VIEW_TIPS
        .iter()
        .map(|s| (*s).into())
        .collect()
}

impl Default for SidebarViewSpec {
    fn default() -> Self {
        Self {
            agents_height: SidebarDimension::Percent("50%".into()),
            comms_height: SidebarDimension::Percent("15%".into()),
            tips_height: SidebarDimension::Percent("35%".into()),
            comms_limit: 5,
            comms_compact: true,
            tips_enabled: true,
            tips: default_tips(),
        }
    }
}

impl SidebarViewSpec {
    pub fn validate(&self) -> crate::Result<()> {
        if self.comms_limit == 0 {
            return Err(crate::AgentError::Validation(
                "ui.sidebar.view.comms_limit must be a positive integer".into(),
            ));
        }
        for tip in &self.tips {
            if tip.trim().is_empty() {
                return Err(crate::AgentError::Validation(
                    "ui.sidebar.view.tips cannot contain empty strings".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "agents_height": self.agents_height.as_string(),
            "comms_height": self.comms_height.as_string(),
            "tips_height": self.tips_height.as_string(),
            "comms_limit": self.comms_limit,
            "comms_compact": self.comms_compact,
            "tips_enabled": self.tips_enabled,
            "tips": self.tips,
        })
    }
}

// --- Window specs ---

fn window_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z][A-Za-z0-9_-]*$").unwrap())
}

pub fn validate_window_name(value: &str) -> crate::Result<String> {
    let name = value.trim();
    if !window_name_re().is_match(name) {
        return Err(crate::AgentError::Validation(format!(
            "invalid window name {name:?}; expected ^[A-Za-z][A-Za-z0-9_-]*$"
        )));
    }
    Ok(name.into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSpec {
    pub name: String,
    #[serde(default)]
    pub order: u32,
    pub layout_spec: String,
    #[serde(default)]
    pub agent_names: Vec<String>,
}

impl WindowSpec {
    pub fn validate(&self) -> crate::Result<()> {
        validate_window_name(&self.name)?;
        if self.layout_spec.trim().is_empty() {
            return Err(crate::AgentError::Validation(format!(
                "windows.{} layout cannot be empty",
                self.name
            )));
        }
        if self.agent_names.is_empty() {
            return Err(crate::AgentError::Validation(format!(
                "windows.{} must contain at least one agent",
                self.name
            )));
        }
        let mut seen = HashSet::new();
        for name in &self.agent_names {
            let n = normalize_agent_name(name)?;
            if !seen.insert(n.clone()) {
                return Err(crate::AgentError::Validation(format!(
                    "windows.{} cannot contain duplicate agents",
                    self.name
                )));
            }
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "order": self.order,
            "layout_spec": self.layout_spec,
            "agent_names": self.agent_names,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolWindowSpec {
    pub name: String,
    #[serde(default)]
    pub order: u32,
    pub command: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "default_true")]
    pub show_in_sidebar: bool,
}

impl ToolWindowSpec {
    pub fn validate(&self) -> crate::Result<()> {
        validate_window_name(&self.name)?;
        if self.command.trim().is_empty() {
            return Err(crate::AgentError::Validation(format!(
                "tool_windows.{}.command cannot be empty",
                self.name
            )));
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "order": self.order,
            "command": self.command,
            "label": self.label,
            "show_in_sidebar": self.show_in_sidebar,
        })
    }
}

// --- Agent spec ---

pub const PROVIDER_COMMAND_PLACEHOLDER: &str = "{command}";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub name: String,
    pub provider: String,
    pub target: String,
    pub workspace_mode: WorkspaceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    pub runtime_mode: RuntimeMode,
    pub restore_default: RestoreMode,
    pub permission_default: PermissionMode,
    pub queue_policy: QueuePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_command_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub startup_args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub api: AgentApiSpec,
    #[serde(default)]
    pub provider_profile: ProviderProfileSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_template: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default)]
    pub watch_paths: Vec<String>,
}

impl AgentSpec {
    pub fn validate(&self) -> crate::Result<()> {
        if normalize_agent_name(&self.name).is_err() {
            return Err(crate::AgentError::Validation("name cannot be empty".into()));
        }
        if self.provider.trim().is_empty() {
            return Err(crate::AgentError::Validation(
                "provider cannot be empty".into(),
            ));
        }
        if self.target.trim().is_empty() {
            return Err(crate::AgentError::Validation(
                "target cannot be empty".into(),
            ));
        }
        Ok(())
    }

    pub fn normalize(&mut self) -> crate::Result<()> {
        self.name = normalize_agent_name(&self.name)?;
        self.provider = self.provider.trim().to_lowercase();
        if self.provider.is_empty() {
            return Err(crate::AgentError::Validation(
                "provider cannot be empty".into(),
            ));
        }
        self.target = self.target.trim().to_string();
        if self.target.is_empty() {
            return Err(crate::AgentError::Validation(
                "target cannot be empty".into(),
            ));
        }
        if let Some(root) = &self.workspace_root {
            let trimmed = root.trim();
            if trimmed.is_empty() {
                return Err(crate::AgentError::Validation(
                    "workspace_root cannot be empty".into(),
                ));
            }
            self.workspace_root = Some(trimmed.into());
        }
        if let Some(path) = &self.workspace_path {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                return Err(crate::AgentError::Validation(
                    "workspace_path cannot be empty".into(),
                ));
            }
            self.workspace_path = Some(trimmed.into());
        }
        if let Some(group) = &self.workspace_group {
            self.workspace_group = Some(normalize_agent_name(group)?);
        }
        if let Some(tpl) = &self.provider_command_template {
            let trimmed = tpl.trim();
            if trimmed.is_empty() {
                return Err(crate::AgentError::Validation(
                    "provider_command_template cannot be empty".into(),
                ));
            }
            if trimmed.matches(PROVIDER_COMMAND_PLACEHOLDER).count() != 1 {
                return Err(crate::AgentError::ProviderCore(
                    ccb_provider_core::error::ProviderCoreError::InvalidCommandTemplate,
                ));
            }
            self.provider_command_template = Some(trimmed.into());
        }
        if let Some(model) = &self.model {
            let trimmed = model.trim();
            if trimmed.is_empty() {
                return Err(crate::AgentError::Validation(
                    "model cannot be empty".into(),
                ));
            }
            self.model = Some(canonical_role_id(trimmed));
        }
        self.startup_args = self.startup_args.iter().map(|s| s.to_string()).collect();
        self.labels = self.labels.iter().map(|s| s.to_string()).collect();
        if let Some(role) = &self.role {
            let trimmed = role.trim().to_lowercase();
            if trimmed.is_empty() {
                return Err(crate::AgentError::Validation("role cannot be empty".into()));
            }
            let allowed: HashSet<char> =
                "abcdefghijklmnopqrstuvwxyz0123456789._-".chars().collect();
            if trimmed.chars().any(|c| !allowed.contains(&c)) || !trimmed.contains('.') {
                return Err(crate::AgentError::Validation(
                    "role must use publisher.role form, for example ccb.archi".into(),
                ));
            }
            self.role = Some(canonical_role_id(&trimmed));
        }
        self.watch_paths = self.watch_paths.iter().map(|s| s.to_string()).collect();
        self.env = self
            .env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.api.normalize();
        self.provider_profile = self.provider_profile.normalized();
        self.validate_workspace_overrides()?;
        self.normalize_startup_args()?;
        Ok(())
    }

    fn validate_workspace_overrides(&self) -> crate::Result<()> {
        let path_set = self.workspace_path.is_some();
        let group_set = self.workspace_group.is_some();
        let root_set = self.workspace_root.is_some();
        if path_set && group_set {
            return Err(crate::AgentError::Validation(
                "workspace_path and workspace_group are mutually exclusive".into(),
            ));
        }
        if path_set && root_set {
            return Err(crate::AgentError::Validation(
                "workspace_path cannot be combined with workspace_root".into(),
            ));
        }
        if group_set && root_set {
            return Err(crate::AgentError::Validation(
                "workspace_group cannot be combined with workspace_root".into(),
            ));
        }
        if (path_set || group_set) && self.workspace_mode != WorkspaceMode::GitWorktree {
            return Err(crate::AgentError::Validation(
                "workspace_path and workspace_group require workspace_mode=\"git-worktree\"".into(),
            ));
        }
        if (path_set || group_set) && self.branch_template.is_some() {
            return Err(crate::AgentError::Validation(
                "workspace_path and workspace_group cannot be combined with branch_template".into(),
            ));
        }
        Ok(())
    }

    fn normalize_startup_args(&mut self) -> crate::Result<()> {
        if let Some(model) = &self.model {
            let compiled = ccb_provider_core::model_shortcuts::provider_model_startup_args(
                &self.provider,
                model,
            )?;
            let normalized: Vec<String> = self.startup_args.clone();
            if normalized.starts_with(&compiled) {
                let remaining = &normalized[compiled.len()..];
                if ccb_provider_core::model_shortcuts::startup_args_contain_model_flag(
                    &self.provider,
                    remaining,
                ) {
                    return Err(crate::AgentError::Validation(format!(
                        "model cannot be combined with startup_args model flags for provider {}",
                        self.provider
                    )));
                }
            } else {
                if ccb_provider_core::model_shortcuts::startup_args_contain_model_flag(
                    &self.provider,
                    &normalized,
                ) {
                    return Err(crate::AgentError::Validation(format!(
                        "model cannot be combined with startup_args model flags for provider {}",
                        self.provider
                    )));
                }
                let mut combined = compiled;
                combined.extend(normalized);
                self.startup_args = combined;
            }
        }
        Ok(())
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "agent_spec",
            "name": self.name,
            "provider": self.provider,
            "target": self.target,
            "workspace_mode": self.workspace_mode,
            "workspace_root": self.workspace_root,
            "workspace_path": self.workspace_path,
            "workspace_group": self.workspace_group,
            "provider_command_template": self.provider_command_template,
            "runtime_mode": self.runtime_mode,
            "restore_default": self.restore_default,
            "permission_default": self.permission_default,
            "queue_policy": self.queue_policy,
            "model": self.model,
            "startup_args": self.startup_args,
            "env": self.env,
            "api": self.api.to_record(),
            "provider_profile": self.provider_profile.to_record(),
            "branch_template": self.branch_template,
            "labels": self.labels,
            "description": self.description,
            "role": self.role,
            "watch_paths": self.watch_paths,
        })
    }
}

// --- Project config ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: u32,
    #[serde(default)]
    pub default_agents: Vec<String>,
    #[serde(default)]
    pub agents: HashMap<String, AgentSpec>,
    #[serde(default)]
    pub cmd_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_spec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<Vec<WindowSpec>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_windows: Option<Vec<ToolWindowSpec>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<SidebarSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar_view: Option<SidebarViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_heartbeat: Option<MaintenanceHeartbeatConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows_explicit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            default_agents: vec![
                "agent1".into(),
                "agent2".into(),
                "agent3".into(),
                "ccb_self".into(),
            ],
            agents: HashMap::new(),
            cmd_enabled: false,
            layout_spec: None,
            windows: None,
            tool_windows: None,
            entry_window: None,
            sidebar: None,
            sidebar_view: None,
            maintenance_heartbeat: None,
            windows_explicit: None,
            topology_signature: None,
            source_path: None,
        }
    }
}

impl ProjectConfig {
    pub fn validate(&self) -> crate::Result<()> {
        if self.version != SCHEMA_VERSION {
            return Err(crate::AgentError::Validation(format!(
                "version must be {SCHEMA_VERSION}"
            )));
        }
        if self.agents.is_empty() {
            return Err(crate::AgentError::Validation(
                "at least one agent must be configured".into(),
            ));
        }
        if self.default_agents.is_empty() {
            return Err(crate::AgentError::Validation(
                "default_agents cannot be empty".into(),
            ));
        }
        let mut seen_defaults = HashSet::new();
        for name in &self.default_agents {
            let n = normalize_agent_name(name)?;
            if !self.agents.contains_key(&n) {
                return Err(crate::AgentError::Validation(format!(
                    "default_agents reference unknown agents: {n}"
                )));
            }
            if !seen_defaults.insert(n.clone()) {
                return Err(crate::AgentError::Validation(
                    "default_agents cannot contain duplicates".into(),
                ));
            }
        }
        let mut seen = HashSet::new();
        for (key, spec) in &self.agents {
            let n = normalize_agent_name(key)?;
            if n != spec.name {
                return Err(crate::AgentError::Validation(format!(
                    "agent key {n:?} does not match spec name {:?}",
                    spec.name
                )));
            }
            if !seen.insert(n.clone()) {
                return Err(crate::AgentError::Validation(format!(
                    "duplicate agent {n:?}"
                )));
            }
        }
        if let Some(windows) = &self.windows {
            if windows.is_empty() {
                return Err(crate::AgentError::Validation(
                    "at least one window must be configured".into(),
                ));
            }
            let mut seen_windows = HashSet::new();
            let mut seen_agents = HashSet::new();
            for window in windows {
                window.validate()?;
                if !seen_windows.insert(window.name.clone()) {
                    return Err(crate::AgentError::Validation(format!(
                        "duplicate window name: {}",
                        window.name
                    )));
                }
                for agent in &window.agent_names {
                    if !self.agents.contains_key(agent) {
                        return Err(crate::AgentError::Validation(format!(
                            "windows reference unknown agents: {agent}"
                        )));
                    }
                    if !seen_agents.insert(agent.clone()) {
                        return Err(crate::AgentError::Validation(format!(
                            "duplicate agent across windows: {agent}"
                        )));
                    }
                }
            }
            let configured: HashSet<String> = self.agents.keys().cloned().collect();
            if seen_agents != configured {
                let unused: Vec<String> = configured.difference(&seen_agents).cloned().collect();
                if !unused.is_empty() {
                    return Err(crate::AgentError::Validation(format!(
                        "configured agents missing from windows: {}",
                        unused.join(", ")
                    )));
                }
            }
            let entry = self.entry_window.as_deref().unwrap_or(&windows[0].name);
            let entry = validate_window_name(entry)?;
            let names: HashSet<String> = windows.iter().map(|w| w.name.clone()).collect();
            if !names.contains(&entry) {
                return Err(crate::AgentError::Validation(format!(
                    "entry_window references unknown window: {entry}"
                )));
            }
        } else {
            if self.entry_window.is_some() {
                return Err(crate::AgentError::Validation(
                    "entry_window requires windows topology".into(),
                ));
            }
            if self.sidebar.is_some() || self.sidebar_view.is_some() {
                return Err(crate::AgentError::Validation(
                    "ui.sidebar requires windows topology".into(),
                ));
            }
            if self
                .tool_windows
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            {
                return Err(crate::AgentError::Validation(
                    "tool_windows requires windows topology".into(),
                ));
            }
            let layout = self.layout_spec.as_deref().unwrap_or("");
            let expected: Vec<String> = self.default_agents.clone();
            let node = parse_layout_spec(layout)?;
            let names: Vec<String> = node.iter_leaves().iter().map(|l| l.name.clone()).collect();
            let mut expected_set: HashSet<String> = expected.iter().cloned().collect();
            if self.cmd_enabled {
                expected_set.insert("cmd".into());
            }
            let name_set: HashSet<String> = names.iter().cloned().collect();
            if name_set != expected_set {
                return Err(crate::AgentError::Validation(
                    "layout_spec must include each configured agent exactly once".into(),
                ));
            }
            if names.len() != name_set.len() {
                return Err(crate::AgentError::Validation(
                    "layout_spec cannot contain duplicate leaves".into(),
                ));
            }
            if self.cmd_enabled && names.first().map(|s| s.as_str()) != Some("cmd") {
                return Err(crate::AgentError::Validation(
                    "layout_spec must anchor cmd as the first pane when cmd_enabled=true".into(),
                ));
            }
        }
        if let Some(tools) = &self.tool_windows {
            let mut seen = HashSet::new();
            for tool in tools {
                tool.validate()?;
                if !seen.insert(tool.name.clone()) {
                    return Err(crate::AgentError::Validation(format!(
                        "duplicate tool window name: {}",
                        tool.name
                    )));
                }
            }
        }
        if let Some(sidebar) = &self.sidebar {
            sidebar.validate()?;
        }
        if let Some(sidebar_view) = &self.sidebar_view {
            sidebar_view.validate()?;
        }
        if let Some(heartbeat) = &self.maintenance_heartbeat {
            heartbeat.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&mut self) -> crate::Result<()> {
        if self.version != SCHEMA_VERSION {
            return Err(crate::AgentError::Validation(format!(
                "version must be {SCHEMA_VERSION}"
            )));
        }
        let mut normalized_agents: HashMap<String, AgentSpec> = HashMap::new();
        let mut key_map: HashMap<String, String> = HashMap::new();
        for (key, spec) in self.agents.drain() {
            let mut s = spec;
            s.normalize()?;
            let n = s.name.clone();
            if key_map.contains_key(&n) {
                return Err(crate::AgentError::Validation(format!(
                    "duplicate agent {n:?}"
                )));
            }
            key_map.insert(n.clone(), key);
            normalized_agents.insert(n, s);
        }
        self.agents = normalized_agents;

        self.default_agents = self
            .default_agents
            .iter()
            .map(|s| normalize_agent_name(s))
            .collect::<crate::Result<Vec<String>>>()?;

        if let Some(windows) = &mut self.windows {
            self.windows_explicit = Some(true);
            if self.layout_spec.is_some() {
                return Err(crate::AgentError::Validation(
                    "layout is not supported with windows topology".into(),
                ));
            }
            if self.cmd_enabled {
                return Err(crate::AgentError::Validation(
                    "cmd_enabled is not supported with windows topology".into(),
                ));
            }
            for (i, window) in windows.iter_mut().enumerate() {
                window.order = i as u32;
                window.validate()?;
            }
            let entry = self
                .entry_window
                .clone()
                .unwrap_or_else(|| windows[0].name.clone());
            self.entry_window = Some(validate_window_name(&entry)?);
            self.layout_spec = None;
        } else {
            self.windows_explicit = Some(false);
            if self
                .layout_spec
                .as_ref()
                .map(|s| s.trim())
                .unwrap_or("")
                .is_empty()
            {
                let providers: HashMap<String, String> = self
                    .agents
                    .iter()
                    .map(|(k, v)| (k.clone(), v.provider.clone()))
                    .collect();
                let worktree_modes: HashMap<String, String> = self
                    .agents
                    .iter()
                    .filter(|(_, v)| v.workspace_mode == WorkspaceMode::GitWorktree)
                    .map(|(k, _)| (k.clone(), "git-worktree".into()))
                    .collect();
                let layout = build_balanced_layout(
                    &self.default_agents,
                    Some(&providers),
                    Some(&worktree_modes),
                    self.cmd_enabled,
                );
                self.layout_spec = Some(layout.render());
            }
        }
        if self.windows.is_some() {
            if self.sidebar.is_none() {
                self.sidebar = Some(SidebarSpec::default());
            }
            if self.sidebar_view.is_none() {
                self.sidebar_view = Some(SidebarViewSpec::default());
            }
        }
        if self.maintenance_heartbeat.is_none() {
            self.maintenance_heartbeat = Some(MaintenanceHeartbeatConfig::default());
        }
        self.topology_signature = Some(self.compute_topology_signature()?);
        self.validate()?;
        Ok(())
    }

    fn compute_topology_signature(&self) -> crate::Result<String> {
        let windows = self.windows.clone().unwrap_or_default();
        let tool_windows = self.tool_windows.clone().unwrap_or_default();
        let entry = self
            .entry_window
            .clone()
            .unwrap_or_else(|| windows.first().map(|w| w.name.clone()).unwrap_or_default());
        let payload = serde_json::json!({
            "version": 1,
            "windows": windows.iter().map(|w| serde_json::json!({
                "name": w.name,
                "order": w.order,
                "layout": w.layout_spec,
                "agents": w.agent_names,
            })).collect::<Vec<_>>(),
            "tool_windows": tool_windows.iter().map(|t| serde_json::json!({
                "name": t.name,
                "order": t.order,
                "command": t.command,
            })).collect::<Vec<_>>(),
            "entry_window": entry,
            "sidebar": self.sidebar.as_ref().map(|s| s.to_record()),
        });
        let encoded = serde_json::to_string(&payload).map_err(crate::AgentError::Json)?;
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(encoded.as_bytes());
        Ok(hex::encode(hash))
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "record_type": "project_config",
            "version": self.version,
            "default_agents": self.default_agents,
            "agents": self.agents.iter().map(|(k, v)| (k.clone(), v.to_record())).collect::<serde_json::Map<String, serde_json::Value>>(),
            "cmd_enabled": self.cmd_enabled,
            "layout_spec": self.layout_spec,
            "windows": self.windows.as_ref().map(|w| w.iter().map(|x| x.to_record()).collect::<Vec<_>>()),
            "tool_windows": self.tool_windows.as_ref().map(|t| t.iter().map(|x| x.to_record()).collect::<Vec<_>>()),
            "entry_window": self.entry_window,
            "sidebar": self.sidebar.as_ref().map(|s| s.to_record()),
            "sidebar_view": self.sidebar_view.as_ref().map(|s| s.to_record()),
            "maintenance": {
                "heartbeat": self.maintenance_heartbeat.as_ref().map(|h| h.to_record())
            },
            "windows_explicit": self.windows_explicit,
            "topology_signature": self.topology_signature,
            "source_path": self.source_path,
        })
    }
}

// --- Layout plan ---

#[derive(Debug, Clone)]
pub struct ProjectLayoutPlan {
    pub target_agent_names: Vec<String>,
    pub visible_leaf_names: Vec<String>,
    pub layout: LayoutNode,
    pub signature: String,
    pub cmd_enabled: bool,
}

pub fn select_project_layout_targets(
    config: &ProjectConfig,
    requested_agents: &[String],
) -> crate::Result<Vec<String>> {
    if requested_agents.is_empty() {
        return Ok(config.default_agents.clone());
    }
    let mut selected = Vec::new();
    let known: HashSet<String> = config.agents.keys().cloned().collect();
    for item in requested_agents {
        let n = normalize_agent_name(item)?;
        if !known.contains(&n) {
            return Err(crate::AgentError::Validation(format!(
                "unknown agent: {item}"
            )));
        }
        if !selected.contains(&n) {
            selected.push(n);
        }
    }
    Ok(selected)
}

pub fn build_project_layout_plan(
    config: &ProjectConfig,
    requested_agents: &[String],
    target_agent_names: Option<&[String]>,
) -> crate::Result<ProjectLayoutPlan> {
    let targets = if let Some(names) = target_agent_names {
        names
            .iter()
            .map(|s| normalize_agent_name(s))
            .collect::<crate::Result<Vec<String>>>()?
    } else {
        select_project_layout_targets(config, requested_agents)?
    };
    let layout_source = config.layout_spec.clone().unwrap_or_default();
    let mut include_names: Vec<String> = if config.cmd_enabled {
        vec!["cmd".into()]
    } else {
        Vec::new()
    };
    include_names.extend(targets.clone());
    let layout = parse_layout_spec(&layout_source)?;
    let pruned = prune_layout(&layout, &include_names).ok_or_else(|| {
        crate::AgentError::Validation(
            "layout_spec does not include any visible panes for the requested start".into(),
        )
    })?;
    let visible_leaf_names: Vec<String> = pruned
        .iter_leaves()
        .iter()
        .map(|l| l.name.clone())
        .collect();
    if config.cmd_enabled && visible_leaf_names.first().map(|s| s.as_str()) != Some("cmd") {
        return Err(crate::AgentError::Validation(
            "pruned layout must retain cmd as the first visible pane".into(),
        ));
    }
    let signature = pruned.render();
    Ok(ProjectLayoutPlan {
        target_agent_names: targets,
        visible_leaf_names,
        layout: pruned,
        signature,
        cmd_enabled: config.cmd_enabled,
    })
}

pub fn project_layout_signature(
    config: &ProjectConfig,
    requested_agents: &[String],
    target_agent_names: Option<&[String]>,
) -> crate::Result<String> {
    Ok(build_project_layout_plan(config, requested_agents, target_agent_names)?.signature)
}

// --- Runtime / restore state ---

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRuntime {
    pub agent_name: String,
    pub state: AgentState,
    pub pid: Option<i64>,
    pub started_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub workspace_path: Option<String>,
    pub project_id: String,
    pub backend_type: String,
    pub queue_depth: i64,
    pub socket_path: Option<String>,
    pub health: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub runtime_root: Option<String>,
    #[serde(default)]
    pub runtime_pid: Option<i64>,
    #[serde(default)]
    pub terminal_backend: Option<String>,
    #[serde(default)]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub active_pane_id: Option<String>,
    #[serde(default)]
    pub pane_title_marker: Option<String>,
    #[serde(default)]
    pub pane_state: Option<String>,
    #[serde(default)]
    pub tmux_socket_name: Option<String>,
    #[serde(default)]
    pub tmux_socket_path: Option<String>,
    #[serde(default)]
    pub tmux_window_name: Option<String>,
    #[serde(default)]
    pub tmux_window_id: Option<String>,
    #[serde(default)]
    pub session_file: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub slot_key: Option<String>,
    #[serde(default)]
    pub window_id: Option<String>,
    #[serde(default)]
    pub workspace_epoch: Option<i64>,
    #[serde(default)]
    pub lifecycle_state: Option<String>,
    #[serde(default = "default_binding_generation")]
    pub binding_generation: u32,
    #[serde(default = "default_managed_by")]
    pub managed_by: String,
    #[serde(default)]
    pub binding_source: RuntimeBindingSource,
    #[serde(default)]
    pub daemon_generation: Option<i64>,
    #[serde(default)]
    pub runtime_generation: Option<i64>,
    #[serde(default)]
    pub desired_state: Option<String>,
    #[serde(default)]
    pub reconcile_state: Option<String>,
    #[serde(default)]
    pub restart_count: u32,
    #[serde(default)]
    pub last_reconcile_at: Option<String>,
    #[serde(default)]
    pub last_failure_reason: Option<String>,
    #[serde(default)]
    pub mount_attempt_id: Option<String>,
}

fn default_binding_generation() -> u32 {
    1
}
fn default_managed_by() -> String {
    "ccbd".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRestoreState {
    pub restore_mode: RestoreMode,
    pub last_checkpoint: Option<String>,
    pub conversation_summary: String,
    #[serde(default)]
    pub open_tasks: Vec<String>,
    #[serde(default)]
    pub files_touched: Vec<String>,
    #[serde(default)]
    pub base_commit: Option<String>,
    #[serde(default)]
    pub head_commit: Option<String>,
    #[serde(default)]
    pub last_restore_status: Option<RestoreStatus>,
}

impl AgentRestoreState {
    pub fn validate(&self) -> crate::Result<()> {
        if self.conversation_summary.trim().is_empty() && self.last_checkpoint.is_none() {
            return Err(crate::AgentError::Validation(
                "conversation_summary cannot be empty when last_checkpoint is missing".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_mode_serde() {
        assert_eq!(
            serde_json::to_string(&WorkspaceMode::GitWorktree).unwrap(),
            "\"git-worktree\""
        );
        let mode: WorkspaceMode = serde_json::from_str("\"copy\"").unwrap();
        assert_eq!(mode, WorkspaceMode::Copy);
    }

    #[test]
    fn test_runtime_mode_default() {
        assert_eq!(RuntimeMode::default(), RuntimeMode::PaneBacked);
    }

    #[test]
    fn test_agent_spec_validate() {
        let spec = AgentSpec {
            name: "agent-a".into(),
            provider: "claude".into(),
            target: "bash".into(),
            workspace_mode: WorkspaceMode::Inplace,
            workspace_root: None,
            runtime_mode: RuntimeMode::PaneBacked,
            restore_default: RestoreMode::Fresh,
            permission_default: PermissionMode::Manual,
            queue_policy: QueuePolicy::SerialPerAgent,
            workspace_path: None,
            workspace_group: None,
            provider_command_template: None,
            model: None,
            startup_args: vec![],
            env: Default::default(),
            api: AgentApiSpec::default(),
            provider_profile: ProviderProfileSpec::default(),
            branch_template: None,
            labels: vec![],
            description: None,
            role: None,
            watch_paths: vec![],
        };
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn test_agent_state_default() {
        assert_eq!(AgentState::default(), AgentState::Starting);
    }

    #[test]
    fn test_project_config_serde() {
        let config = ProjectConfig {
            version: SCHEMA_VERSION,
            default_agents: vec!["agent-a".into()],
            agents: HashMap::new(),
            cmd_enabled: false,
            layout_spec: None,
            windows: None,
            tool_windows: None,
            entry_window: None,
            sidebar: None,
            sidebar_view: None,
            maintenance_heartbeat: None,
            windows_explicit: None,
            topology_signature: None,
            source_path: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.version, SCHEMA_VERSION);
    }
}
