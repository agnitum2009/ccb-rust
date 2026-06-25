use std::collections::HashMap;

use ccbr_terminal::panes::{TmuxPaneService, TmuxRunOutput};

fn cp(stdout: &str, returncode: i32) -> TmuxRunOutput {
    TmuxRunOutput {
        stdout: stdout.to_string(),
        stderr: String::new(),
        returncode,
    }
}

#[test]
fn test_tmux_pane_service_gets_current_pane_and_finds_marker() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
    let calls_clone = calls.clone();
    let service = TmuxPaneService::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            calls_clone
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            if args == ["display-message", "-p", "-t", "%1", "#{pane_id}"] {
                return Ok(cp("%1\n", 0));
            }
            if args == ["list-panes", "-a", "-F", "#{pane_id}\t#{pane_title}"] {
                return Ok(cp("%1\tCCBR-one\n%2\tOTHER\n", 0));
            }
            Ok(cp("%1\n", 0))
        },
    );

    assert_eq!(service.get_current_pane_id("%1").unwrap(), "%1");
    assert_eq!(service.find_pane_by_title_marker("CCBR").unwrap(), "%1");
}

#[test]
fn test_tmux_pane_service_sets_user_option_and_reads_content() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
    let calls_clone = calls.clone();
    let service = TmuxPaneService::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            calls_clone
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            if args.len() >= 2 && args[0] == "capture-pane" && args[1] == "-t" {
                return Ok(cp("\x1b[31mhello\x1b[0m\n", 0));
            }
            if args.len() >= 2
                && args[0] == "display-message"
                && args[1] == "-p"
                && args.contains(&"#{pane_dead}")
            {
                return Ok(cp("0\n", 0));
            }
            Ok(cp("", 0))
        },
    );

    service.set_pane_user_option("%3", "ccbr_agent", "Gemini");
    let text = service.get_pane_content("%3", 20);
    let alive = service.is_pane_alive("%3");

    assert_eq!(
        calls.lock().unwrap()[0],
        vec!["set-option", "-p", "-t", "%3", "@ccb_agent", "Gemini"]
    );
    assert_eq!(text, Some("hello\n".to_string()));
    assert!(alive);
}

#[test]
fn test_tmux_pane_service_describes_pane_with_user_options() {
    let service =
        TmuxPaneService::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                if args == [
                    "display-message",
                    "-p",
                    "-t",
                    "%3",
                    "#{pane_id}\t#{pane_title}\t#{pane_dead}\t#{@ccbr_agent}\t#{@ccbr_project_id}",
                ] {
                    return Ok(cp("%3\tagent2\t0\tagent2\tproj-1\n", 0));
                }
                Ok(cp("", 1))
            },
        );

    let described = service.describe_pane("%3", &["@ccb_agent".into(), "@ccb_project_id".into()]);

    assert_eq!(
        described,
        Some(HashMap::from_iter([
            ("pane_id".to_string(), "%3".to_string()),
            ("pane_title".to_string(), "agent2".to_string()),
            ("pane_dead".to_string(), "0".to_string()),
            ("@ccb_agent".to_string(), "agent2".to_string()),
            ("@ccb_project_id".to_string(), "proj-1".to_string()),
        ]))
    );
}

#[test]
fn test_tmux_pane_service_finds_unique_pane_by_user_options() {
    let service = TmuxPaneService::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            if args
                == [
                    "list-panes",
                    "-a",
                    "-F",
                    "#{pane_id}\t#{@ccbr_agent}\t#{@ccbr_project_id}",
                ]
            {
                return Ok(cp("%1\tagent1\tproj-1\n%2\tagent1\tproj-2\n", 0));
            }
            Ok(cp("", 0))
        },
    );

    let mut expected = HashMap::new();
    expected.insert("ccbr_agent".to_string(), "agent1".to_string());
    expected.insert("ccbr_project_id".to_string(), "proj-2".to_string());
    assert_eq!(
        service.find_pane_by_user_options(&expected),
        Some("%2".to_string())
    );
}

#[test]
fn test_tmux_pane_service_lists_matching_panes_by_user_options() {
    let service = TmuxPaneService::new(
        move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
            if args == ["list-panes", "-a", "-F", "#{pane_id}\t#{@ccbr_project_id}"] {
                return Ok(cp("%1\tproj-1\n%2\tproj-2\n%3\tproj-2\n", 0));
            }
            Ok(cp("", 0))
        },
    );

    let mut expected = HashMap::new();
    expected.insert("ccbr_project_id".to_string(), "proj-2".to_string());
    assert_eq!(
        service.list_panes_by_user_options(&expected),
        vec!["%2".to_string(), "%3".to_string()]
    );
}
