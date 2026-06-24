use ccb_terminal::tmux::{
    default_detached_session_name, looks_like_pane_id, looks_like_tmux_target,
    normalize_socket_name, normalize_split_direction, pane_id_by_title_marker_output,
    socket_name_from_tmux_env, tmux_base,
};

#[test]
fn test_tmux_base_includes_socket_when_present() {
    std::env::remove_var("CCB_TMUX_CONFIG");
    assert_eq!(tmux_base(None, None), vec!["tmux", "-f", "/dev/null"]);
    assert_eq!(
        tmux_base(Some("ccb-demo"), None),
        vec!["tmux", "-f", "/dev/null", "-L", "ccb-demo"]
    );
    let with_path = tmux_base(None, Some("~/.tmux/demo.sock"));
    assert_eq!(with_path[0..4], vec!["tmux", "-f", "/dev/null", "-S"]);
    assert!(with_path[4].ends_with(".tmux/demo.sock"));
}

#[test]
fn test_tmux_base_allows_managed_config_override() {
    std::env::set_var("CCB_TMUX_CONFIG", "~/.config/ccb/tmux.conf");
    let base = tmux_base(None, None);
    assert_eq!(base[0], "tmux");
    assert_eq!(base[1], "-f");
    assert!(base[2].ends_with(".config/ccb/tmux.conf"));
    std::env::remove_var("CCB_TMUX_CONFIG");
}

#[test]
fn test_tmux_target_helpers() {
    assert!(looks_like_pane_id("%1"));
    assert!(!looks_like_pane_id("sess"));
    assert!(looks_like_tmux_target("%1"));
    assert!(looks_like_tmux_target("sess:1.0"));
    assert!(!looks_like_tmux_target("sess"));
}

#[test]
fn test_tmux_socket_name_helpers() {
    assert_eq!(normalize_socket_name(None), None);
    assert_eq!(normalize_socket_name(Some("")), None);
    assert_eq!(normalize_socket_name(Some("default")), None);
    assert_eq!(normalize_socket_name(Some("ccb")), Some("ccb".to_string()));
    assert_eq!(socket_name_from_tmux_env(None), None);
    assert_eq!(socket_name_from_tmux_env(Some("")), None);
    assert_eq!(
        socket_name_from_tmux_env(Some("/tmp/tmux-1000/default,123,0")),
        None
    );
    assert_eq!(
        socket_name_from_tmux_env(Some("/tmp/tmux-1000/ccb,123,0")),
        Some("ccb".to_string())
    );
}

#[test]
fn test_normalize_split_direction() {
    assert_eq!(normalize_split_direction("right"), ("-h", "right"));
    assert_eq!(normalize_split_direction("vertical"), ("-v", "bottom"));
}

#[test]
#[should_panic(expected = "unsupported direction")]
fn test_normalize_split_direction_left_panics() {
    normalize_split_direction("left");
}

#[test]
fn test_pane_id_by_title_marker_output() {
    let stdout = "%1\tCCB-a\n%2\tOTHER\n";
    assert_eq!(
        pane_id_by_title_marker_output(stdout, "CCB"),
        Some("%1".to_string())
    );
    assert_eq!(pane_id_by_title_marker_output(stdout, "missing"), None);

    let ambiguous = "%1\tCCB-codex-a1b2c3d4\n%2\tCCB-codex-e5f6g7h8\n";
    assert_eq!(pane_id_by_title_marker_output(ambiguous, "CCB-codex"), None);

    let exact = "%1\tCCB-codex\n%2\tCCB-codex-a1b2c3d4\n";
    assert_eq!(
        pane_id_by_title_marker_output(exact, "CCB-codex"),
        Some("%1".to_string())
    );
}

#[test]
fn test_default_detached_session_name_is_stable_format() {
    let name = default_detached_session_name("/tmp/demo", 123, 1700000000.0);
    assert_eq!(name, "ccb-demo-0-123");
}
