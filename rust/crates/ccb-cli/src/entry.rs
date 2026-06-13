use crate::commands;
use crate::parser::*;
use crate::services::{resolve_project_root, socket_path_for_project, UnixDaemonClient};
use std::path::PathBuf;

pub const VERSION: &str = "7.4.3";

/// Main CLI entry point. Returns exit code.
pub fn run_cli(argv: &[String]) -> i32 {
    if argv
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return 0;
    }
    let cli = match parse_args(argv) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 2;
        }
    };
    dispatch(cli)
}

fn dispatch(cmd: ParsedCommand) -> i32 {
    if let ParsedCommand::Version = cmd {
        println!("ccb {}", VERSION);
        return 0;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_flag = project_for(&cmd).clone();
    let project_root = match resolve_project_root(&cwd, project_flag.as_deref()) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 2;
        }
    };
    let socket_path = socket_path_for_project(&project_root);
    let client = UnixDaemonClient::new(socket_path);

    let result = match cmd {
        ParsedCommand::Start(start) => commands::start(&client, &start),
        ParsedCommand::Kill(kill) => commands::stop(&client, kill.force),
        ParsedCommand::Stop(stop) => commands::stop(&client, stop.force),
        ParsedCommand::StopAll(stop) => commands::stop(&client, stop.force),
        ParsedCommand::Status(_) => commands::status(&client),
        ParsedCommand::ProjectView(_) => commands::status(&client),
        ParsedCommand::Ps(ps) => commands::ps(&client, ps.alive_only),
        ParsedCommand::Attach(attach) => {
            commands::attach(&client, &attach.agent_name, &project_root)
        }
        ParsedCommand::Ask(ask) => {
            let layout = ccb_storage::paths::PathLayout::new(
                camino::Utf8Path::from_path(&project_root).unwrap_or(camino::Utf8Path::new("/")),
            );
            commands::ask(&client, &ask, layout.project_id())
        }
        ParsedCommand::Ping(ping) => commands::ping(&client, &ping.target),
        ParsedCommand::Shutdown(_) => commands::shutdown(&client),
        _ => Ok(format!("Command not yet implemented: {:?}\n", cmd)),
    };

    match result {
        Ok(output) => {
            print!("{}", output);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn project_for(cmd: &ParsedCommand) -> &Option<String> {
    match cmd {
        ParsedCommand::Start(c) => &c.project,
        ParsedCommand::Ask(c) => &c.project,
        ParsedCommand::Wait(c) => &c.project,
        ParsedCommand::Watch(c) => &c.project,
        ParsedCommand::Ps(c) => &c.project,
        ParsedCommand::Ping(c) => &c.project,
        ParsedCommand::Cancel(c) => &c.project,
        ParsedCommand::Clear(c) => &c.project,
        ParsedCommand::Kill(c) => &c.project,
        ParsedCommand::Pend(c) => &c.project,
        ParsedCommand::Queue(c) => &c.project,
        ParsedCommand::Trace(c) => &c.project,
        ParsedCommand::Resubmit(c) => &c.project,
        ParsedCommand::Retry(c) => &c.project,
        ParsedCommand::Inbox(c) => &c.project,
        ParsedCommand::Ack(c) => &c.project,
        ParsedCommand::Logs(c) => &c.project,
        ParsedCommand::Maintenance(c) => &c.project,
        ParsedCommand::Doctor(c) => &c.project,
        ParsedCommand::Reload(c) => &c.project,
        ParsedCommand::Restart(c) => &c.project,
        ParsedCommand::Status(c) => &c.project,
        ParsedCommand::Stop(c) => &c.project,
        ParsedCommand::StopAll(c) => &c.project,
        ParsedCommand::Attach(c) => &c.project,
        ParsedCommand::Shutdown(c) => &c.project,
        ParsedCommand::ProjectView(c) => &c.project,
        _ => &None,
    }
}

fn parse_args(argv: &[String]) -> Result<ParsedCommand, String> {
    let project = extract_project(argv);
    let filtered: Vec<&String> = argv
        .iter()
        .filter(|a| !a.starts_with("--project"))
        .collect();

    if filtered.is_empty() {
        return Ok(ParsedCommand::Start(ParsedStart {
            project,
            ..Default::default()
        }));
    }

    let first = filtered[0].as_str();

    match first {
        "ask" => parse_ask(&filtered[1..], project),
        "wait" => parse_wait(&filtered[1..], project),
        "watch" => Ok(ParsedCommand::Watch(ParsedWatch {
            project,
            target: get(1, &filtered),
        })),
        "ps" => Ok(ParsedCommand::Ps(ParsedPs {
            project,
            alive_only: has("--alive", &filtered),
        })),
        "ping" => Ok(ParsedCommand::Ping(ParsedPing {
            project,
            target: get(1, &filtered),
        })),
        "cancel" => Ok(ParsedCommand::Cancel(ParsedCancel {
            project,
            job_id: get(1, &filtered),
        })),
        "clear" => Ok(ParsedCommand::Clear(ParsedClear {
            project,
            agent_names: filtered[1..].iter().map(|s| s.to_string()).collect(),
        })),
        "cleanup" => Ok(ParsedCommand::Cleanup),
        "kill" => Ok(ParsedCommand::Kill(ParsedKill {
            project,
            force: has("-f", &filtered) || has("--force", &filtered),
        })),
        "stop" => Ok(ParsedCommand::Stop(ParsedStop {
            project,
            force: has("-f", &filtered) || has("--force", &filtered),
        })),
        "stop-all" => Ok(ParsedCommand::StopAll(ParsedStopAll {
            project,
            force: has("-f", &filtered) || has("--force", &filtered),
        })),
        "status" => Ok(ParsedCommand::Status(ParsedStatus { project })),
        "project-view" => Ok(ParsedCommand::ProjectView(ParsedProjectView { project })),
        "attach" => Ok(ParsedCommand::Attach(ParsedAttach {
            project,
            agent_name: get(1, &filtered),
        })),
        "shutdown" => Ok(ParsedCommand::Shutdown(ParsedShutdown { project })),
        "pend" => parse_pend(&filtered[1..], project),
        "queue" => Ok(ParsedCommand::Queue(ParsedQueue {
            project,
            target: get(1, &filtered),
            detail: has("--detail", &filtered),
        })),
        "trace" => Ok(ParsedCommand::Trace(ParsedTrace {
            project,
            target: get(1, &filtered),
        })),
        "resubmit" => Ok(ParsedCommand::Resubmit(ParsedResubmit {
            project,
            message_id: get(1, &filtered),
        })),
        "retry" => Ok(ParsedCommand::Retry(ParsedRetry {
            project,
            target: get(1, &filtered),
        })),
        "inbox" => Ok(ParsedCommand::Inbox(ParsedInbox {
            project,
            agent_name: get(1, &filtered),
            detail: has("--detail", &filtered),
        })),
        "ack" => Ok(ParsedCommand::Ack(ParsedAck {
            project,
            agent_name: get(1, &filtered),
            event_id: filtered.get(2).map(|s| s.to_string()),
        })),
        "logs" => Ok(ParsedCommand::Logs(ParsedLogs {
            project,
            agent_name: get(1, &filtered),
        })),
        "maintenance" => Ok(ParsedCommand::Maintenance(ParsedMaintenance {
            project,
            action: filtered
                .get(1)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "status".into()),
        })),
        "doctor" => Ok(ParsedCommand::Doctor(ParsedDoctor {
            project,
            bundle: has("--bundle", &filtered),
            output_path: position("--output", &filtered)
                .and_then(|i| filtered.get(i + 1))
                .map(|s| s.to_string()),
            storage: has("--storage", &filtered),
            json_output: has("--json", &filtered),
        })),
        "reload" => Ok(ParsedCommand::Reload(ParsedReload {
            project,
            dry_run: has("--dry-run", &filtered),
        })),
        "restart" => Ok(ParsedCommand::Restart(ParsedRestart {
            project,
            agent_name: get(1, &filtered),
        })),
        "version" | "-v" | "--version" => Ok(ParsedCommand::Version),
        "update" => Ok(ParsedCommand::Update),
        "uninstall" => Ok(ParsedCommand::Uninstall),
        "reinstall" => Ok(ParsedCommand::Reinstall),
        "config" => Ok(ParsedCommand::ConfigValidate),
        other => {
            if other.starts_with('-') {
                parse_start(&filtered, project)
            } else {
                // Unknown token: treat as a start command with agent names.
                parse_start(&filtered, project)
            }
        }
    }
}

fn get(i: usize, filtered: &[&String]) -> String {
    filtered.get(i).map(|s| s.to_string()).unwrap_or_default()
}

fn has(s: &str, filtered: &[&String]) -> bool {
    filtered.iter().any(|a| a.as_str() == s)
}

fn position(s: &str, filtered: &[&String]) -> Option<usize> {
    filtered.iter().position(|a| a.as_str() == s)
}

fn extract_project(argv: &[String]) -> Option<String> {
    for (i, arg) in argv.iter().enumerate() {
        if arg == "--project" {
            return argv.get(i + 1).cloned();
        }
        if let Some(value) = arg.strip_prefix("--project=") {
            return Some(value.to_string());
        }
    }
    None
}

fn parse_ask(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let mut sender: Option<String> = None;
    let mut task_id: Option<String> = None;
    let mut compact = false;
    let mut silence = false;
    let mut positional: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        match token {
            "--from" => {
                if i + 1 >= args.len() {
                    return Err("--from requires a value".to_string());
                }
                sender = Some(args[i + 1].clone());
                i += 2;
            }
            "--task-id" => {
                if i + 1 >= args.len() {
                    return Err("--task-id requires a value".to_string());
                }
                task_id = Some(args[i + 1].clone());
                i += 2;
            }
            "--compact" => {
                compact = true;
                i += 1;
            }
            "--silence" => {
                silence = true;
                i += 1;
            }
            _ => {
                positional.push(token);
                i += 1;
            }
        }
    }

    let target = positional
        .first()
        .map(|s| s.to_string())
        .ok_or("ask requires a target")?;
    let message = if positional.len() > 1 {
        positional[1..].join(" ")
    } else {
        String::new()
    };

    Ok(ParsedCommand::Ask(ParsedAsk {
        project,
        target,
        sender,
        message,
        task_id,
        compact,
        silence,
    }))
}

fn parse_wait(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let target = args
        .first()
        .map(|s| s.to_string())
        .ok_or("wait requires a target")?;
    Ok(ParsedCommand::Wait(ParsedWait {
        project,
        target,
        quorum: None,
        timeout_s: None,
    }))
}

fn parse_pend(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let target = args.first().map(|s| s.to_string()).unwrap_or_default();
    let count = args.get(1).and_then(|s| s.parse().ok());
    let has = |s: &str| args.iter().any(|a| a.as_str() == s);
    Ok(ParsedCommand::Pend(ParsedPend {
        project,
        target,
        count,
        watch: has("--watch"),
        inbox: has("--inbox"),
        queue: has("--queue"),
        detail: has("--detail"),
    }))
}

fn parse_start(tokens: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let mut auto_permission = true;
    let mut reset_context = false;
    let mut agent_names: Vec<String> = Vec::new();

    for token in tokens {
        match token.as_str() {
            "-s" | "--safe" => auto_permission = false,
            "-n" | "--new-context" | "--reset" => reset_context = true,
            t if t.starts_with('-') => {}
            t => agent_names.push(t.to_string()),
        }
    }

    Ok(ParsedCommand::Start(ParsedStart {
        project,
        agent_names,
        restore: !reset_context,
        auto_permission,
        reset_context,
    }))
}

fn print_help() {
    println!("ccb {} - CCB multi-agent CLI workspace", VERSION);
    println!();
    println!("Usage: ccb [OPTIONS] [COMMAND] [ARGS...]");
    println!();
    println!("Options:");
    println!("  -h, --help                 Print this help message");
    println!("  -v, --version              Print version information");
    println!("      --project <PATH>       Use the CCB project at <PATH>");
    println!();
    println!("Commands:");
    println!("  start [agents...]          Start the workspace and/or agents (default)");
    println!("  ask <agent> [message]      Send a message to an agent");
    println!("  status                     Show workspace status");
    println!("  ps [--alive]               List agents/processes");
    println!("  attach <agent>             Attach to an agent pane");
    println!("  stop [--force]             Stop the workspace");
    println!("  stop-all [--force]         Stop all workspaces");
    println!("  kill [--force]             Force-kill the workspace");
    println!("  shutdown                   Shut down the CCB daemon");
    println!("  ping <target>              Ping an agent or service");
    println!("  wait <target>              Wait for a condition");
    println!("  watch <target>             Watch a target");
    println!("  cancel <job-id>            Cancel a job");
    println!("  clear <agent>...           Clear agent context");
    println!("  queue [--detail] <target>  Show the message queue");
    println!("  trace <target>             Trace a target");
    println!("  resubmit <message-id>      Resubmit a message");
    println!("  retry <target>             Retry a target");
    println!("  inbox [--detail] <agent>   Show agent inbox");
    println!("  ack <agent> [event-id]     Acknowledge an event");
    println!("  logs <agent>               Show agent logs");
    println!("  maintenance [action]       Maintenance operations");
    println!("  doctor [--bundle]          Run diagnostics");
    println!("  reload [--dry-run]         Reload configuration");
    println!("  restart <agent>            Restart an agent");
    println!("  project-view               Show project view");
    println!("  cleanup                    Clean up workspace artifacts");
    println!("  config validate            Validate configuration");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let args = vec!["version".to_string()];
        let cmd = parse_args(&args).unwrap();
        assert!(matches!(cmd, ParsedCommand::Version));
    }

    #[test]
    fn test_parse_empty_is_start() {
        let args: Vec<String> = vec![];
        let cmd = parse_args(&args).unwrap();
        assert!(matches!(cmd, ParsedCommand::Start(_)));
    }

    #[test]
    fn test_parse_start_safe() {
        let args = vec!["-s".to_string()];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Start(s) = cmd {
            assert!(!s.auto_permission);
            assert!(s.agent_names.is_empty());
        } else {
            panic!("expected Start");
        }
    }

    #[test]
    fn test_parse_start_agents() {
        let args = vec!["claude".to_string(), "gemini".to_string()];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Start(s) = cmd {
            assert_eq!(s.agent_names, vec!["claude", "gemini"]);
        } else {
            panic!("expected Start");
        }
    }

    #[test]
    fn test_parse_kill_force() {
        let args = vec!["kill".to_string(), "-f".to_string()];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Kill(k) = cmd {
            assert!(k.force);
        } else {
            panic!("expected Kill");
        }
    }

    #[test]
    fn test_parse_stop() {
        let args = vec!["stop".to_string(), "--force".to_string()];
        let cmd = parse_args(&args).unwrap();
        assert!(matches!(cmd, ParsedCommand::Stop(_)));
    }

    #[test]
    fn test_parse_ask() {
        let args = vec![
            "ask".to_string(),
            "agent-a".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Ask(a) = cmd {
            assert_eq!(a.target, "agent-a");
            assert_eq!(a.message, "hello world");
        } else {
            panic!("expected Ask");
        }
    }

    #[test]
    fn test_parse_ask_from() {
        let args = vec![
            "ask".to_string(),
            "agent-a".to_string(),
            "--from".to_string(),
            "codex".to_string(),
            "hello".to_string(),
        ];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Ask(a) = cmd {
            assert_eq!(a.sender, Some("codex".to_string()));
            assert_eq!(a.message, "hello");
        } else {
            panic!("expected Ask");
        }
    }

    #[test]
    fn test_parse_ps_alive() {
        let args = vec!["ps".to_string(), "--alive".to_string()];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Ps(p) = cmd {
            assert!(p.alive_only);
        } else {
            panic!("expected Ps");
        }
    }

    #[test]
    fn test_parse_unknown_command_treated_as_start() {
        let args = vec!["bogus".to_string()];
        let cmd = parse_args(&args).unwrap();
        assert!(matches!(cmd, ParsedCommand::Start(_)));
    }

    #[test]
    fn test_dispatch_version() {
        let code = dispatch(ParsedCommand::Version);
        assert_eq!(code, 0);
    }
}
