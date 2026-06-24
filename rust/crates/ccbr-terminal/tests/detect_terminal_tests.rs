//! Mirrors Python `test/test_detect_terminal.py`.

use std::collections::HashMap;

use ccbr_terminal::detect::detect_terminal_with;

fn fake_run(args: &[&str]) -> Option<String> {
    if args
        .windows(5)
        .any(|w| w == ["display-message", "-p", "-t", "%1", "#{pane_tty}"])
    {
        return Some("/dev/pts/7".to_string());
    }
    if args
        .windows(4)
        .any(|w| w == ["display-message", "-p", "#{client_tty}"])
    {
        return Some("/dev/pts/7".to_string());
    }
    if args
        .windows(5)
        .any(|w| w == ["display-message", "-p", "-t", "%1", "#{pane_id}"])
    {
        return Some("%1".to_string());
    }
    None
}

#[test]
fn test_detect_terminal_prefers_current_tmux_session() {
    let mut env = HashMap::new();
    env.insert(
        "TMUX".to_string(),
        "/tmp/tmux-1000/default,123,0".to_string(),
    );
    env.insert("TMUX_PANE".to_string(), "%1".to_string());
    env.insert("TERM".to_string(), "xterm-256color".to_string());

    let result = detect_terminal_with(
        &env,
        |name| {
            if name == "tmux" {
                Some("/usr/bin/tmux".to_string())
            } else {
                None
            }
        },
        fake_run,
        || Some("/dev/pts/7".to_string()),
    );

    assert_eq!(result, Some("tmux".to_string()));
}

#[test]
fn test_detect_terminal_does_not_select_tmux_when_not_inside_tmux() {
    let env = HashMap::new();

    let result = detect_terminal_with(
        &env,
        |_name| Some("/usr/bin/tmux".to_string()),
        |_args| unreachable!("should not run tmux"),
        || Some("/dev/pts/7".to_string()),
    );

    assert_eq!(result, None);
}

#[test]
fn test_detect_terminal_rejects_stale_tmux_env() {
    let mut env = HashMap::new();
    env.insert(
        "TMUX".to_string(),
        "/tmp/tmux-1000/default,123,0".to_string(),
    );
    env.insert("TMUX_PANE".to_string(), "%1".to_string());

    let result = detect_terminal_with(
        &env,
        |name| {
            if name == "tmux" {
                Some("/usr/bin/tmux".to_string())
            } else {
                None
            }
        },
        fake_run,
        || Some("/dev/pts/9".to_string()),
    );

    assert_eq!(result, None);
}
