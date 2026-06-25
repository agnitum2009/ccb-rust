//! Tmux theme/profile system.
//!
//! Mirrors Python `terminal_runtime.tmux_theme`.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

const DEFAULT_FALLBACK_LABEL_STYLE: &str = "#[fg=#1e1e2e]#[bg=#7aa2f7]#[bold]";
const STATUS_STYLE: &str = "bg=#1e1e2e fg=#cdd6f4";
const STATUS_FORMAT_1: &str =
    "#[align=centre,bg=#1e1e2e,fg=#6c7086]Copy: MouseDrag  Paste: Shift-Ctrl-v  Focus: Ctrl-b o";
const STATUS_FORMAT_0: &str = "#[align=left bg=#1e1e2e]#{T:status-left}#[align=centre fg=#6c7086]#{b:pane_current_path}#[align=right]#{T:status-right}";
const WINDOW_STATUS_FORMAT: &str = "";
const WINDOW_STATUS_CURRENT_FORMAT: &str = "";
const WINDOW_STATUS_SEPARATOR: &str = "";
const PANE_BORDER_STATUS: &str = "top";
const CONTRAST_TERMINAL_FAMILIES: &[&str] = &["apple_terminal"];

/// Visual styling for a CCB pane.
pub use crate::identity::TmuxPaneVisual;

/// A named tmux theme profile.
#[derive(Debug, Clone)]
pub struct TmuxThemeProfile {
    pub name: String,
    pub fallback_label_style: String,
    pub pane_border_style: String,
    pub pane_active_border_style: String,
    pub window_style: Option<String>,
    pub window_active_style: Option<String>,
}

/// Rendered session- and window-level tmux options for a theme.
#[derive(Debug, Clone)]
pub struct RenderedTmuxSessionTheme {
    pub profile_name: String,
    pub session_options: HashMap<String, String>,
    pub window_options: HashMap<String, String>,
}

fn theme_profiles() -> HashMap<String, TmuxThemeProfile> {
    let mut profiles = HashMap::new();
    profiles.insert(
        "default".to_string(),
        TmuxThemeProfile {
            name: "default".to_string(),
            fallback_label_style: DEFAULT_FALLBACK_LABEL_STYLE.to_string(),
            pane_border_style: "fg=#3b4261,bold".to_string(),
            pane_active_border_style: "fg=#7aa2f7,bold".to_string(),
            window_style: None,
            window_active_style: None,
        },
    );
    profiles.insert(
        "contrast".to_string(),
        TmuxThemeProfile {
            name: "contrast".to_string(),
            fallback_label_style: DEFAULT_FALLBACK_LABEL_STYLE.to_string(),
            pane_border_style: "fg=#565f89,bold".to_string(),
            pane_active_border_style: "fg=#89b4fa,bold".to_string(),
            window_style: Some("bg=#181825".to_string()),
            window_active_style: Some("bg=#1e1e2e".to_string()),
        },
    );
    profiles
}

fn visual(bg: &str, border: Option<&str>, active: Option<&str>, fg: &str) -> TmuxPaneVisual {
    let border_color = border.unwrap_or(bg).trim();
    let active_color = active.unwrap_or(border_color).trim();
    TmuxPaneVisual {
        label_style: format!("#[fg={fg}]#[bg={bg}]#[bold]"),
        border_style: format!("fg={border_color}"),
        active_border_style: format!("fg={active_color},bold"),
    }
}

fn cmd_visuals_default() -> Vec<TmuxPaneVisual> {
    vec![
        visual("#7dcfff", Some("#5fb3d6"), Some("#7dcfff"), "#16161e"),
        visual("#73daca", Some("#4fb7a9"), Some("#73daca"), "#16161e"),
        visual("#89b4fa", Some("#6b8fd6"), Some("#89b4fa"), "#16161e"),
        visual("#2ac3de", Some("#1b9fb8"), Some("#2ac3de"), "#16161e"),
    ]
}

fn agent_visuals_default() -> Vec<TmuxPaneVisual> {
    vec![
        visual("#ff9e64", Some("#d9824f"), Some("#ff9e64"), "#16161e"),
        visual("#9ece6a", Some("#7ca952"), Some("#9ece6a"), "#16161e"),
        visual("#f7768e", Some("#d85f78"), Some("#f7768e"), "#16161e"),
        visual("#e0af68", Some("#bd8d4f"), Some("#e0af68"), "#16161e"),
        visual("#bb9af7", Some("#9d7fda"), Some("#bb9af7"), "#16161e"),
        visual("#73daca", Some("#54bda7"), Some("#73daca"), "#16161e"),
        visual("#7aa2f7", Some("#5d82d6"), Some("#7aa2f7"), "#16161e"),
        visual("#f6bd60", Some("#d69f46"), Some("#f6bd60"), "#16161e"),
        visual("#ff757f", Some("#da5a66"), Some("#ff757f"), "#16161e"),
        visual("#8bd5ca", Some("#68b6aa"), Some("#8bd5ca"), "#16161e"),
        visual("#c6a0f6", Some("#a885d8"), Some("#c6a0f6"), "#16161e"),
        visual("#a6da95", Some("#84b777"), Some("#a6da95"), "#16161e"),
        TmuxPaneVisual {
            label_style: "#[fg=#16161e]#[bg=#f5bde6]#[bold]".to_string(),
            border_style: "fg=#d49ac5".to_string(),
            active_border_style: "fg=#f5bde6,bold".to_string(),
        },
    ]
}

fn cmd_visuals_contrast() -> Vec<TmuxPaneVisual> {
    vec![
        visual("#7dcfff", None, None, "#16161e"),
        visual("#73daca", None, None, "#16161e"),
        visual("#89b4fa", None, None, "#16161e"),
        visual("#2ac3de", None, None, "#16161e"),
    ]
}

fn agent_visuals_contrast() -> Vec<TmuxPaneVisual> {
    vec![
        visual("#ff9e64", None, None, "#16161e"),
        visual("#9ece6a", None, None, "#16161e"),
        visual("#f7768e", None, None, "#16161e"),
        visual("#e0af68", None, None, "#16161e"),
        visual("#bb9af7", None, None, "#16161e"),
        visual("#73daca", None, None, "#16161e"),
        visual("#7aa2f7", None, None, "#16161e"),
        visual("#f6bd60", None, None, "#16161e"),
        visual("#ff757f", None, None, "#16161e"),
        visual("#8bd5ca", None, None, "#16161e"),
        visual("#c6a0f6", None, None, "#16161e"),
        visual("#a6da95", None, None, "#16161e"),
        visual("#f5bde6", None, None, "#16161e"),
    ]
}

fn sidebar_visual() -> TmuxPaneVisual {
    TmuxPaneVisual {
        label_style: "#[fg=#cdd6f4]#[bg=#45475a]#[bold]".to_string(),
        border_style: "fg=#6c7086".to_string(),
        active_border_style: "fg=#6c7086".to_string(),
    }
}

fn env_map(
    environ: Option<&HashMap<String, String>>,
) -> std::borrow::Cow<'_, HashMap<String, String>> {
    match environ {
        Some(e) => std::borrow::Cow::Borrowed(e),
        None => std::borrow::Cow::Owned(std::env::vars().collect()),
    }
}

/// Detect the terminal family from environment variables.
pub fn detect_terminal_family(environ: Option<&HashMap<String, String>>) -> String {
    let env = env_map(environ);
    for key in ["TERM_PROGRAM", "LC_TERMINAL"] {
        if let Some(value) = env.get(key) {
            let value = value.trim().to_lowercase();
            if !value.is_empty() {
                return value;
            }
        }
    }
    env.get("TERM")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default()
}

fn normalize_profile_name(value: Option<&str>) -> Option<String> {
    let name = value.unwrap_or("").trim().to_lowercase();
    if name.is_empty() {
        return None;
    }
    let profiles = theme_profiles();
    if profiles.contains_key(&name) {
        Some(name)
    } else {
        None
    }
}

/// Resolve the active tmux theme profile name.
pub fn tmux_theme_profile(environ: Option<&HashMap<String, String>>) -> String {
    let env = env_map(environ);
    if let Some(override_name) =
        normalize_profile_name(env.get("CCB_TMUX_THEME_PROFILE").map(|s| s.as_str()))
    {
        return override_name;
    }
    let family = detect_terminal_family(Some(&env));
    if CONTRAST_TERMINAL_FAMILIES.contains(&family.as_str()) {
        "contrast".to_string()
    } else {
        "default".to_string()
    }
}

/// Resolve the tmux status interval.
pub fn tmux_status_interval(environ: Option<&HashMap<String, String>>) -> String {
    let env = env_map(environ);
    let raw = env
        .get("CCB_TMUX_STATUS_INTERVAL")
        .map(|s| s.trim())
        .unwrap_or("");
    if let Ok(n) = raw.parse::<i64>() {
        if n > 0 {
            return n.to_string();
        }
    }
    "5".to_string()
}

/// Resolve a theme profile definition by name.
pub fn theme_profile_definition(
    profile_name: Option<&str>,
    environ: Option<&HashMap<String, String>>,
) -> TmuxThemeProfile {
    let resolved =
        normalize_profile_name(profile_name).unwrap_or_else(|| tmux_theme_profile(environ));
    let profiles = theme_profiles();
    profiles
        .get(&resolved)
        .cloned()
        .unwrap_or_else(|| profiles["default"].clone())
}

/// Build the `pane-border-format` option value.
pub fn pane_border_format(
    profile_name: Option<&str>,
    environ: Option<&HashMap<String, String>>,
) -> String {
    let profile = theme_profile_definition(profile_name, environ);
    format!(
        "#{{?#{{@ccb_agent}},#{{?#{{@ccb_label_style}},#{{@ccb_label_style}},{fallback}}} #{{@ccb_agent}} #[default],#[fg=#565f89] #{{pane_title}} #[default]}}",
        fallback = profile.fallback_label_style
    )
}

/// Render the full tmux session theme.
pub fn render_tmux_session_theme(
    ccb_version: &str,
    status_script: Option<&str>,
    git_script: Option<&str>,
    environ: Option<&HashMap<String, String>>,
    profile_name: Option<&str>,
) -> RenderedTmuxSessionTheme {
    let profile = theme_profile_definition(profile_name, environ);
    let normalized_version = normalized_ccb_version(ccb_version);
    let focus_agent = "#{?#{@ccb_agent},#{@ccb_agent},-}";
    let accent = "#{?client_prefix,#f38ba8,#{?pane_in_mode,#fab387,#f5c2e7}}";
    let label = "#{?client_prefix,KEY,#{?pane_in_mode,COPY,INPUT}}";
    let git_info = git_script
        .map(|s| format!("#({} \"#{{pane_current_path}}\")", s))
        .unwrap_or_else(|| "-".to_string());
    let status_indicator = status_script
        .map(|s| format!("#({} modern \"#{{pane_current_path}}\")", s))
        .unwrap_or_else(|| "-".to_string());

    let mut session_options = HashMap::new();
    session_options.insert("@ccb_active".to_string(), "1".to_string());
    session_options.insert("@ccb_version".to_string(), normalized_version.clone());
    session_options.insert("@ccb_theme_profile".to_string(), profile.name.clone());
    session_options.insert("status-position".to_string(), "bottom".to_string());
    session_options.insert("status-interval".to_string(), tmux_status_interval(environ));
    session_options.insert("status-style".to_string(), STATUS_STYLE.to_string());
    session_options.insert("status".to_string(), "2".to_string());
    session_options.insert("status-left-length".to_string(), "80".to_string());
    session_options.insert("status-right-length".to_string(), "120".to_string());
    session_options.insert("status-format[1]".to_string(), STATUS_FORMAT_1.to_string());
    session_options.insert("status-format[0]".to_string(), STATUS_FORMAT_0.to_string());
    session_options.insert(
        "status-left".to_string(),
        format!(
            "#[fg=#1e1e2e,bg={accent},bold] {label} #[fg={accent},bg=#cba6f7]#[fg=#1e1e2e,bg=#cba6f7] {git_info} #[fg=#cba6f7,bg=#1e1e2e]"
        ),
    );
    session_options.insert(
        "status-right".to_string(),
        format!(
            "#[fg=#f38ba8,bg=#1e1e2e]#[fg=#1e1e2e,bg=#f38ba8,bold] {focus_agent} #[fg=#cba6f7,bg=#f38ba8]#[fg=#1e1e2e,bg=#cba6f7,bold] CCB:{normalized_version} #[fg=#89b4fa,bg=#cba6f7]#[fg=#cdd6f4,bg=#89b4fa] {status_indicator} #[fg=#fab387,bg=#89b4fa]#[fg=#1e1e2e,bg=#fab387,bold] %m/%d %a %H:%M #[default]"
        ),
    );
    session_options.insert(
        "window-status-format".to_string(),
        WINDOW_STATUS_FORMAT.to_string(),
    );
    session_options.insert(
        "window-status-current-format".to_string(),
        WINDOW_STATUS_CURRENT_FORMAT.to_string(),
    );
    session_options.insert(
        "window-status-separator".to_string(),
        WINDOW_STATUS_SEPARATOR.to_string(),
    );

    let mut window_options = HashMap::new();
    window_options.insert(
        "pane-border-status".to_string(),
        PANE_BORDER_STATUS.to_string(),
    );
    window_options.insert(
        "pane-border-style".to_string(),
        profile.pane_border_style.clone(),
    );
    window_options.insert(
        "pane-active-border-style".to_string(),
        profile.pane_active_border_style.clone(),
    );
    window_options.insert(
        "pane-border-format".to_string(),
        pane_border_format(Some(&profile.name), environ),
    );
    if let Some(style) = &profile.window_style {
        window_options.insert("window-style".to_string(), style.clone());
    }
    if let Some(style) = &profile.window_active_style {
        window_options.insert("window-active-style".to_string(), style.clone());
    }

    RenderedTmuxSessionTheme {
        profile_name: profile.name,
        session_options,
        window_options,
    }
}

fn pane_palette(profile_name: &str, is_cmd: bool) -> Vec<TmuxPaneVisual> {
    if profile_name == "contrast" {
        if is_cmd {
            cmd_visuals_contrast()
        } else {
            agent_visuals_contrast()
        }
    } else if is_cmd {
        cmd_visuals_default()
    } else {
        agent_visuals_default()
    }
}

/// Compute visual style for a pane.
pub fn pane_visual(
    project_id: Option<&str>,
    slot_key: Option<&str>,
    order_index: Option<usize>,
    is_cmd: bool,
    role: Option<&str>,
    profile_name: Option<&str>,
    environ: Option<&HashMap<String, String>>,
) -> TmuxPaneVisual {
    if role.map(|r| r.trim().to_lowercase()).as_deref() == Some("sidebar") {
        return sidebar_visual();
    }
    let resolved_profile = theme_profile_definition(profile_name, environ).name;
    let visuals = pane_palette(&resolved_profile, is_cmd);
    select_visual(&visuals, project_id, slot_key, order_index)
}

fn select_visual(
    visuals: &[TmuxPaneVisual],
    project_id: Option<&str>,
    slot_key: Option<&str>,
    fallback_index: Option<usize>,
) -> TmuxPaneVisual {
    if let (Some(pid), Some(slot)) = (project_id, slot_key) {
        let key = format!("{pid}:{slot}");
        return visuals[stable_index(&key, visuals.len())].clone();
    }
    let index = fallback_index.unwrap_or(0);
    visuals[index % visuals.len()].clone()
}

fn stable_index(key: &str, size: usize) -> usize {
    if size == 0 {
        return 0;
    }
    let digest = Sha256::digest(key.as_bytes());
    let hex = format!("{:x}", digest);
    let prefix = &hex[..8.min(hex.len())];
    usize::from_str_radix(prefix, 16).unwrap_or(0) % size
}

/// Render theme settings as shell export statements.
pub fn shell_exports(
    ccb_version: &str,
    status_script: Option<&str>,
    git_script: Option<&str>,
    environ: Option<&HashMap<String, String>>,
    profile_name: Option<&str>,
) -> String {
    let rendered = render_tmux_session_theme(
        ccb_version,
        status_script,
        git_script,
        environ,
        profile_name,
    );
    let items: Vec<(&str, String)> = vec![
        ("CCB_TMUX_RENDERED_THEME_PROFILE", rendered.profile_name),
        (
            "CCB_TMUX_RENDERED_STATUS_POSITION",
            rendered
                .session_options
                .get("status-position")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_INTERVAL",
            rendered
                .session_options
                .get("status-interval")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_STYLE",
            rendered
                .session_options
                .get("status-style")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_LINES",
            rendered
                .session_options
                .get("status")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_LEFT_LENGTH",
            rendered
                .session_options
                .get("status-left-length")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_RIGHT_LENGTH",
            rendered
                .session_options
                .get("status-right-length")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_FORMAT_0",
            rendered
                .session_options
                .get("status-format[0]")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_FORMAT_1",
            rendered
                .session_options
                .get("status-format[1]")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_LEFT",
            rendered
                .session_options
                .get("status-left")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_STATUS_RIGHT",
            rendered
                .session_options
                .get("status-right")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_WINDOW_STATUS_FORMAT",
            rendered
                .session_options
                .get("window-status-format")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_WINDOW_STATUS_CURRENT_FORMAT",
            rendered
                .session_options
                .get("window-status-current-format")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_WINDOW_STATUS_SEPARATOR",
            rendered
                .session_options
                .get("window-status-separator")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_PANE_BORDER_STATUS",
            rendered
                .window_options
                .get("pane-border-status")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_PANE_BORDER_STYLE",
            rendered
                .window_options
                .get("pane-border-style")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_PANE_ACTIVE_BORDER_STYLE",
            rendered
                .window_options
                .get("pane-active-border-style")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_PANE_BORDER_FORMAT",
            rendered
                .window_options
                .get("pane-border-format")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_WINDOW_STYLE",
            rendered
                .window_options
                .get("window-style")
                .cloned()
                .unwrap_or_default(),
        ),
        (
            "CCB_TMUX_RENDERED_WINDOW_ACTIVE_STYLE",
            rendered
                .window_options
                .get("window-active-style")
                .cloned()
                .unwrap_or_default(),
        ),
    ];
    items
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, shell_words::quote(&v)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalized_ccb_version(value: &str) -> String {
    let s = value.trim();
    if s.is_empty() {
        "?".to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_terminal_family_reads_term_program() {
        let mut env = HashMap::new();
        env.insert("TERM_PROGRAM".to_string(), "Apple_Terminal".to_string());
        assert_eq!(detect_terminal_family(Some(&env)), "apple_terminal");
    }

    #[test]
    fn test_detect_terminal_family_falls_back_to_term() {
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        assert_eq!(detect_terminal_family(Some(&env)), "xterm-256color");
    }

    #[test]
    fn test_tmux_theme_profile_override() {
        let mut env = HashMap::new();
        env.insert(
            "CCB_TMUX_THEME_PROFILE".to_string(),
            "contrast".to_string(),
        );
        assert_eq!(tmux_theme_profile(Some(&env)), "contrast");
    }

    #[test]
    fn test_tmux_theme_profile_apple_terminal_is_contrast() {
        let mut env = HashMap::new();
        env.insert("TERM_PROGRAM".to_string(), "Apple_Terminal".to_string());
        assert_eq!(tmux_theme_profile(Some(&env)), "contrast");
    }

    #[test]
    fn test_tmux_theme_profile_default() {
        let env = HashMap::new();
        assert_eq!(tmux_theme_profile(Some(&env)), "default");
    }

    #[test]
    fn test_tmux_status_interval() {
        let mut env = HashMap::new();
        env.insert("CCB_TMUX_STATUS_INTERVAL".to_string(), "10".to_string());
        assert_eq!(tmux_status_interval(Some(&env)), "10");
        env.insert("CCB_TMUX_STATUS_INTERVAL".to_string(), "0".to_string());
        assert_eq!(tmux_status_interval(Some(&env)), "5");
        env.insert("CCB_TMUX_STATUS_INTERVAL".to_string(), "abc".to_string());
        assert_eq!(tmux_status_interval(Some(&env)), "5");
    }

    #[test]
    fn test_theme_profile_definition() {
        let profile = theme_profile_definition(Some("contrast"), None);
        assert_eq!(profile.name, "contrast");
        assert_eq!(profile.pane_border_style, "fg=#565f89,bold");
    }

    #[test]
    fn test_pane_border_format_contains_agent_marker() {
        let fmt = pane_border_format(None, None);
        assert!(fmt.contains("@ccb_agent"));
        assert!(fmt.contains("pane_title"));
    }

    #[test]
    fn test_render_tmux_session_theme_has_expected_keys() {
        let rendered = render_tmux_session_theme("7.5.2", None, None, None, None);
        assert_eq!(rendered.profile_name, "default");
        assert!(rendered.session_options.contains_key("status-left"));
        assert!(rendered.session_options.contains_key("status-right"));
        assert!(rendered.window_options.contains_key("pane-border-format"));
        assert!(rendered.window_options.contains_key("pane-border-style"));
    }

    #[test]
    fn test_render_tmux_session_theme_contrast_has_window_style() {
        let rendered = render_tmux_session_theme("7.5.2", None, None, None, Some("contrast"));
        assert_eq!(rendered.profile_name, "contrast");
        assert!(rendered.window_options.contains_key("window-style"));
        assert!(rendered.window_options.contains_key("window-active-style"));
    }

    #[test]
    fn test_pane_visual_sidebar() {
        let visual = pane_visual(None, None, None, false, Some("sidebar"), None, None);
        assert_eq!(visual.border_style, "fg=#6c7086");
    }

    #[test]
    fn test_pane_visual_stable_by_project_and_slot() {
        let v1 = pane_visual(Some("proj"), Some("slot-a"), None, false, None, None, None);
        let v2 = pane_visual(Some("proj"), Some("slot-a"), None, false, None, None, None);
        let v3 = pane_visual(Some("proj"), Some("slot-b"), None, false, None, None, None);
        assert_eq!(v1.label_style, v2.label_style);
        assert_ne!(v1.label_style, v3.label_style);
    }

    #[test]
    fn test_pane_visual_fallback_index() {
        let v0 = pane_visual(None, None, Some(0), false, None, None, None);
        let v1 = pane_visual(None, None, Some(1), false, None, None, None);
        assert_ne!(v0.label_style, v1.label_style);
    }

    #[test]
    fn test_shell_exports_contains_keys() {
        let exports = shell_exports("7.5.2", None, None, None, None);
        assert!(exports.contains("CCB_TMUX_RENDERED_THEME_PROFILE"));
        assert!(exports.contains("CCB_TMUX_RENDERED_STATUS_LEFT"));
        assert!(exports.contains("CCB_TMUX_RENDERED_PANE_BORDER_FORMAT"));
    }

    #[test]
    fn test_normalized_ccb_version() {
        let rendered = render_tmux_session_theme("", None, None, None, None);
        assert_eq!(rendered.session_options["@ccb_version"], "?");
    }
}
