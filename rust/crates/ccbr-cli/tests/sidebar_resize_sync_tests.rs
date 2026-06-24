use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ccbr_cli::sidebar_resize_sync::{sync_sidebar_resize, SidebarResizeSync};

fn tmux_command_args(args: &[String]) -> Vec<String> {
    for (index, value) in args.iter().enumerate() {
        if matches!(
            value.as_str(),
            "list-panes" | "resize-pane" | "set-option" | "show-option"
        ) {
            return args[index..].to_vec();
        }
    }
    args.to_vec()
}

#[test]
fn test_sidebar_resize_sync_copies_source_window_sidebar_width_to_other_windows() {
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();
    let pane_rows = "ccb-demo\t@1\tmain\t%0\t41\t160\tproj-1\tsidebar\tmain\tccbd\n\
                     ccb-demo\t@1\tmain\t%1\t118\t160\tproj-1\tagent\t\tccbd\n\
                     ccb-demo\t@2\twork\t%2\t23\t160\tproj-1\tsidebar\twork\tccbd\n\
                     ccb-demo\t@2\twork\t%3\t136\t160\tproj-1\tagent\t\tccbd\n\
                     ccb-demo\t@3\treview\t%4\t24\t160\tproj-1\tsidebar\treview\tccbd\n\
                     ccb-demo\t@3\treview\t%5\t135\t160\tproj-1\tagent\t\tccbd";

    let count = sync_sidebar_resize(
        &SidebarResizeSync {
            tmux_socket_path: PathBuf::from("/tmp/tmux.sock"),
            session_name: "ccb-demo".to_string(),
            source_pane: "%1".to_string(),
            source_window: String::new(),
            project_id: "proj-1".to_string(),
            from_stored_width: false,
        },
        Some(&|args: &[String]| {
            let tmux_args = tmux_command_args(args);
            calls_clone.lock().unwrap().push(tmux_args.clone());
            if tmux_args.first() == Some(&"list-panes".to_string())
                && tmux_args.get(1) == Some(&"-a".to_string())
            {
                Ok(pane_rows.to_string())
            } else {
                Ok(String::new())
            }
        }),
    );

    let locked = calls.lock().unwrap();
    assert_eq!(count, Some(2));
    assert!(locked.contains(&vec![
        "set-option".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_width_cells".to_string(),
        "41".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "set-option".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_sync_guard".to_string(),
        "1".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "resize-pane".to_string(),
        "-t".to_string(),
        "%2".to_string(),
        "-x".to_string(),
        "41".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "resize-pane".to_string(),
        "-t".to_string(),
        "%4".to_string(),
        "-x".to_string(),
        "41".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "set-option".to_string(),
        "-u".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_sync_guard".to_string(),
    ]));
}

#[test]
fn test_sidebar_resize_sync_noops_when_source_window_has_no_sidebar() {
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();
    let pane_rows = "ccb-demo\t@1\tmain\t%1\t160\t160\tproj-1\tagent\t\tccbd";

    let count = sync_sidebar_resize(
        &SidebarResizeSync {
            tmux_socket_path: PathBuf::from("/tmp/tmux.sock"),
            session_name: "ccb-demo".to_string(),
            source_pane: "%1".to_string(),
            source_window: String::new(),
            project_id: "proj-1".to_string(),
            from_stored_width: false,
        },
        Some(&|args: &[String]| {
            let tmux_args = tmux_command_args(args);
            calls_clone.lock().unwrap().push(tmux_args.clone());
            if tmux_args.first() == Some(&"list-panes".to_string())
                && tmux_args.get(1) == Some(&"-a".to_string())
            {
                Ok(pane_rows.to_string())
            } else {
                Ok(String::new())
            }
        }),
    );

    let locked = calls.lock().unwrap();
    assert_eq!(count, None);
    assert_eq!(locked.len(), 1);
    assert_eq!(
        locked[0].get(0..3),
        Some(vec!["list-panes".to_string(), "-a".to_string(), "-F".to_string(),].as_slice())
    );
}

#[test]
fn test_sidebar_resize_sync_reapplies_stored_width_after_window_resize() {
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();
    let pane_rows = "ccb-demo\t@1\tmain\t%0\t19\t80\tproj-1\tsidebar\tmain\tccbd\n\
                     ccb-demo\t@1\tmain\t%1\t60\t80\tproj-1\tagent\t\tccbd\n\
                     ccb-demo\t@2\twork\t%2\t59\t160\tproj-1\tsidebar\twork\tccbd\n\
                     ccb-demo\t@2\twork\t%3\t100\t160\tproj-1\tagent\t\tccbd";

    let count = sync_sidebar_resize(
        &SidebarResizeSync {
            tmux_socket_path: PathBuf::from("/tmp/tmux.sock"),
            session_name: "ccb-demo".to_string(),
            source_pane: String::new(),
            source_window: "@1".to_string(),
            project_id: String::new(),
            from_stored_width: true,
        },
        Some(&|args: &[String]| {
            let tmux_args = tmux_command_args(args);
            calls_clone.lock().unwrap().push(tmux_args.clone());
            if tmux_args.first() == Some(&"list-panes".to_string())
                && tmux_args.get(1) == Some(&"-a".to_string())
            {
                Ok(pane_rows.to_string())
            } else if tmux_args.first() == Some(&"show-option".to_string())
                && tmux_args.get(1) == Some(&"-qv".to_string())
                && tmux_args.get(2) == Some(&"-t".to_string())
                && tmux_args.get(3) == Some(&"ccb-demo".to_string())
            {
                Ok("59\n".to_string())
            } else {
                Ok(String::new())
            }
        }),
    );

    let locked = calls.lock().unwrap();
    assert_eq!(count, Some(1));
    assert!(locked.contains(&vec![
        "show-option".to_string(),
        "-qv".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_width_cells".to_string(),
    ]));
    assert!(!locked.contains(&vec![
        "set-option".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_width_cells".to_string(),
        "59".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "resize-pane".to_string(),
        "-t".to_string(),
        "%0".to_string(),
        "-x".to_string(),
        "59".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "set-option".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_sync_guard".to_string(),
        "1".to_string(),
    ]));
    assert!(locked.contains(&vec![
        "set-option".to_string(),
        "-u".to_string(),
        "-t".to_string(),
        "ccb-demo".to_string(),
        "@ccbr_sidebar_sync_guard".to_string(),
    ]));
}
