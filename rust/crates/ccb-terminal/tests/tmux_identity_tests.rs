//! Mirrors Python `test/test_tmux_identity.py`.

use std::collections::HashMap;

use ccb_terminal::tmux_identity::pane_visual;
use ccb_terminal::tmux_theme::render_tmux_session_theme;

#[test]
fn test_pane_visual_is_stable_for_same_project_slot() {
    let first = pane_visual(
        Some("proj-1"),
        Some("agent3"),
        None,
        false,
        None,
        None,
        None,
    );
    let second = pane_visual(
        Some("proj-1"),
        Some("agent3"),
        Some(99),
        false,
        None,
        None,
        None,
    );
    assert_eq!(first.label_style, second.label_style);
    assert_eq!(first.border_style, second.border_style);
    assert_eq!(first.active_border_style, second.active_border_style);
}

#[test]
fn test_pane_visual_uses_different_palette_for_cmd_pool() {
    let cmd_visual = pane_visual(Some("proj-1"), Some("cmd"), None, true, None, None, None);
    let agent_visual = pane_visual(Some("proj-1"), Some("cmd"), None, false, None, None, None);
    assert_ne!(cmd_visual.label_style, agent_visual.label_style);
}

#[test]
fn test_pane_visual_uses_order_index_when_slot_identity_missing() {
    let first = pane_visual(None, None, Some(0), false, None, None, None);
    let second = pane_visual(None, None, Some(1), false, None, None, None);
    assert_ne!(first.label_style, second.label_style);
}

#[test]
fn test_render_tmux_session_theme_uses_terminal_profile_overrides() {
    let mut env = HashMap::new();
    env.insert("TERM_PROGRAM".to_string(), "Apple_Terminal".to_string());

    let rendered = render_tmux_session_theme("9.9.9", None, None, Some(&env), None);
    assert_eq!(rendered.profile_name, "contrast");
    assert_eq!(
        rendered.window_options["pane-border-style"],
        "fg=#565f89,bold"
    );
    assert_eq!(rendered.window_options["window-style"], "bg=#181825");
}
