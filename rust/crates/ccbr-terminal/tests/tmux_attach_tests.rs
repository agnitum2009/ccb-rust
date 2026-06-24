use ccbr_terminal::tmux_attach::{
    normalize_user_option, pane_exists_output, pane_is_alive, pane_pipe_enabled,
    parse_session_name, should_attach_selected_pane,
};

#[test]
fn test_tmux_attach_helpers() {
    assert_eq!(normalize_user_option("ccbr_agent"), "@ccbr_agent");
    assert_eq!(normalize_user_option("@keep"), "@keep");
    assert_eq!(normalize_user_option(""), "");
    assert!(pane_exists_output("%12\n"));
    assert!(!pane_exists_output("12\n"));
    assert!(pane_pipe_enabled("1\n"));
    assert!(!pane_pipe_enabled("0\n"));
    assert!(pane_is_alive("0\n"));
    assert!(!pane_is_alive("1\n"));
    assert_eq!(parse_session_name(" demo \n"), "demo");
    assert!(should_attach_selected_pane(""));
    assert!(!should_attach_selected_pane("/tmp/tmux"));
}
