use crate::commands;
use crate::parser::*;
use crate::services::{resolve_project_root, socket_path_for_project, UnixDaemonClient};
use crate::source_guard::source_runtime_allowed;
use std::path::PathBuf;

pub const VERSION: &str = "7.5.2";

/// Main CLI entry point. Returns exit code.
pub fn run_cli(argv: &[String]) -> i32 {
    if argv
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return 0;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let guard = source_runtime_allowed(argv, &cwd);
    if !guard.allowed {
        eprintln!("{}", guard.reason);
        return 1;
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
        println!("ccbr {}", VERSION);
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
            let layout = ccbr_storage::paths::PathLayout::new(
                camino::Utf8Path::from_path(&project_root).unwrap_or(camino::Utf8Path::new("/")),
            );
            commands::ask(&client, &ask, layout.project_id())
        }
        ParsedCommand::Ping(ping) => commands::ping(&client, &ping.target),
        ParsedCommand::Shutdown(_) => commands::shutdown(&client),
        ParsedCommand::Wait(wait) => commands::wait(&client, &wait),
        ParsedCommand::Watch(watch) => commands::watch(&client, &watch),
        ParsedCommand::Cancel(cancel) => commands::cancel(&client, &cancel),
        ParsedCommand::Clear(clear) => commands::clear(&client, &clear),
        ParsedCommand::Queue(queue) => commands::queue(&client, &queue),
        ParsedCommand::Trace(trace) => commands::trace(&client, &trace),
        ParsedCommand::Resubmit(resubmit) => commands::resubmit(&client, &resubmit),
        ParsedCommand::Retry(retry) => commands::retry(&client, &retry),
        ParsedCommand::Inbox(inbox) => commands::inbox(&client, &inbox),
        ParsedCommand::Ack(ack) => commands::ack(&client, &ack),
        ParsedCommand::Reload(reload) => commands::reload(&client, &reload),
        ParsedCommand::Restart(restart) => commands::restart(&client, &restart),
        ParsedCommand::Maintenance(maintenance) => {
            let context_command = crate::models::ParsedCommand::Maintenance(
                crate::models::ParsedMaintenanceCommand::new(maintenance.project.clone()),
            );
            let context = match crate::context::CliContextBuilder::new(context_command)
                .cwd(cwd.clone())
                .build()
            {
                Ok(ctx) => ctx,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return 2;
                }
            };
            commands::maintenance(&client, &maintenance, &context)
        }
        ParsedCommand::Logs(logs) => commands::logs(&client, &logs),
        ParsedCommand::Cleanup(_) => commands::cleanup(&client),
        ParsedCommand::Doctor(doctor) => commands::doctor(&client, &doctor, &project_root),
        ParsedCommand::ConfigValidate(config) => commands::config_validate(&config, &project_root),
        ParsedCommand::Pend(pend) => commands::pend(&client, &pend),
        ParsedCommand::Tools(tools) => commands::tools(&tools),
        ParsedCommand::Roles(roles) => commands::roles(&roles, &project_root),
        ParsedCommand::Fault(fault) => commands::fault(&client, &fault),
        ParsedCommand::Repair(repair) => commands::repair(&client, &repair),
        ParsedCommand::Update(_) => commands::update(),
        ParsedCommand::Uninstall(_) => commands::uninstall(),
        ParsedCommand::Reinstall(_) => commands::reinstall(),
        ParsedCommand::Autonew(autonew) => commands::autonew(&autonew, &project_root),
        ParsedCommand::CtxTransfer(ctx) => commands::ctx_transfer(&ctx, &project_root),
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
        ParsedCommand::Cleanup(c) => &c.project,
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
        ParsedCommand::ConfigValidate(c) => &c.project,
        ParsedCommand::Tools(c) => &c.project,
        ParsedCommand::Roles(c) => &c.project,
        ParsedCommand::Fault(c) => &c.project,
        ParsedCommand::Repair(c) => &c.project,
        ParsedCommand::Reload(c) => &c.project,
        ParsedCommand::Restart(c) => &c.project,
        ParsedCommand::Status(c) => &c.project,
        ParsedCommand::Stop(c) => &c.project,
        ParsedCommand::StopAll(c) => &c.project,
        ParsedCommand::Attach(c) => &c.project,
        ParsedCommand::Shutdown(c) => &c.project,
        ParsedCommand::ProjectView(c) => &c.project,
        ParsedCommand::Update(c) => &c.project,
        ParsedCommand::Uninstall(c) => &c.project,
        ParsedCommand::Reinstall(c) => &c.project,
        ParsedCommand::Autonew(c) => &c.project,
        ParsedCommand::CtxTransfer(c) => &c.project,
        _ => &None,
    }
}

fn parse_args(argv: &[String]) -> Result<ParsedCommand, String> {
    let project = extract_project(argv);
    let filtered: Vec<&String> = argv
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            if a.starts_with("--project=") {
                return false;
            }
            if *a == "--project" {
                return false;
            }
            // Also drop the value that follows `--project`.
            if *i > 0 && argv[*i - 1] == "--project" {
                return false;
            }
            true
        })
        .map(|(_, a)| a)
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
        "cleanup" => Ok(ParsedCommand::Cleanup(ParsedCleanup { project })),
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
            args: filtered[2..].iter().map(|s| s.to_string()).collect(),
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
        "update" => Ok(ParsedCommand::Update(ParsedUpdate { project })),
        "uninstall" => Ok(ParsedCommand::Uninstall(ParsedUninstall { project })),
        "reinstall" => Ok(ParsedCommand::Reinstall(ParsedReinstall { project })),
        "config" => parse_config(&filtered[1..], project),
        "tools" => parse_tools(&filtered[1..], project),
        "roles" => parse_roles(&filtered[1..], project),
        "fault" => parse_fault(&filtered[1..], project),
        "repair" => parse_repair(&filtered[1..], project),
        "autonew" => parse_autonew(&filtered[1..], project),
        "ctx-transfer" => parse_ctx_transfer(&filtered[1..], project),
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

fn parse_config(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    if args.first().map(|s| s.as_str()) != Some("validate") {
        return Err("config only supports: ccbr config validate".to_string());
    }
    Ok(ParsedCommand::ConfigValidate(ParsedConfigValidate {
        project,
    }))
}

fn parse_tools(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let action = match args.first().map(|s| s.as_str()) {
        Some("doctor") => ToolsAction::Doctor {
            tool: args
                .get(1)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "neovim".into()),
        },
        Some("install") => ToolsAction::Install {
            tool: args
                .get(1)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "neovim".into()),
        },
        _ => return Err("tools supports: doctor, install".to_string()),
    };
    Ok(ParsedCommand::Tools(ParsedTools { project, action }))
}

fn parse_roles(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let action = match args.first().map(|s| s.as_str()) {
        Some("list") => RolesAction::List,
        Some("add") => RolesAction::Add {
            spec: args.get(1).map(|s| s.to_string()).unwrap_or_default(),
        },
        Some("update") => RolesAction::Update {
            path: args.get(1).map(|s| s.to_string()).unwrap_or_default(),
        },
        Some("install") => RolesAction::Install {
            path: args.get(1).map(|s| s.to_string()).unwrap_or_default(),
        },
        Some("sync") => RolesAction::Sync {
            path: args.get(1).map(|s| s.to_string()),
        },
        Some("doctor") => RolesAction::Doctor {
            path: args.get(1).map(|s| s.to_string()).unwrap_or_default(),
        },
        _ => return Err("roles supports: list, add, update, install, sync, doctor".to_string()),
    };
    Ok(ParsedCommand::Roles(ParsedRoles { project, action }))
}

fn parse_fault(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let action = match args.first().map(|s| s.as_str()) {
        Some("list") => FaultAction::List,
        Some("arm") => {
            let agent_name = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            let task_id =
                position_value("--task-id", args).ok_or("fault arm requires --task-id")?;
            let reason = position_value("--reason", args).unwrap_or_else(|| "api_error".into());
            let count = position_value("--count", args)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            let error = position_value("--error", args);
            FaultAction::Arm {
                agent_name,
                task_id,
                reason: Some(reason),
                count,
                error,
            }
        }
        Some("clear") => FaultAction::Clear {
            target: args.get(1).map(|s| s.to_string()).unwrap_or_default(),
        },
        _ => return Err("fault supports: list, arm, clear".to_string()),
    };
    Ok(ParsedCommand::Fault(ParsedFault { project, action }))
}

fn parse_repair(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let mode = args.first().map(|s| s.as_str()).unwrap_or("");
    let rest = &args[1..];
    let action = match mode {
        "ack" => RepairAction::Ack {
            target: rest.first().map(|s| s.to_string()).unwrap_or_default(),
            event_id: rest.get(1).map(|s| s.to_string()),
        },
        "retry" => RepairAction::Retry {
            target: rest.first().map(|s| s.to_string()).unwrap_or_default(),
        },
        "resubmit" => RepairAction::Resubmit {
            target: rest.first().map(|s| s.to_string()).unwrap_or_default(),
        },
        _ => return Err("repair supports: ack, retry, resubmit".to_string()),
    };
    Ok(ParsedCommand::Repair(ParsedRepair { project, action }))
}

fn position_value(s: &str, args: &[&String]) -> Option<String> {
    position(s, args)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

fn parse_autonew(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    if args.first().map(|s| s.as_str()) == Some("-h")
        || args.first().map(|s| s.as_str()) == Some("--help")
    {
        return Ok(ParsedCommand::Autonew(ParsedAutonew {
            project,
            provider: "-h".to_string(),
        }));
    }
    let provider = args
        .first()
        .map(|s| s.to_string())
        .ok_or("autonew requires a provider")?;
    Ok(ParsedCommand::Autonew(ParsedAutonew { project, provider }))
}

fn parse_ctx_transfer(args: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let mut last: usize = 3;
    let mut source_provider = "auto".to_string();
    let mut agent_name: Option<String> = None;
    let mut send = false;
    let mut dry_run = false;
    let mut output: Option<String> = None;
    let mut session_path: Option<String> = None;
    let mut max_tokens: usize = 8000;
    let mut format = "markdown".to_string();
    let mut quiet = false;
    let mut save = false;
    let mut no_save = false;
    let mut detailed = false;

    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        match token {
            "-n" | "--last" => {
                if i + 1 >= args.len() {
                    return Err("--last requires a value".to_string());
                }
                last = args[i + 1].parse().map_err(|_| "invalid --last value")?;
                i += 2;
            }
            "--from" => {
                if i + 1 >= args.len() {
                    return Err("--from requires a value".to_string());
                }
                source_provider = args[i + 1].clone();
                i += 2;
            }
            "--agent" => {
                if i + 1 >= args.len() {
                    return Err("--agent requires a value".to_string());
                }
                agent_name = Some(args[i + 1].clone());
                i += 2;
            }
            "--send" => {
                send = true;
                i += 1;
            }
            "-d" | "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "-o" | "--output" => {
                if i + 1 >= args.len() {
                    return Err("--output requires a value".to_string());
                }
                output = Some(args[i + 1].clone());
                i += 2;
            }
            "--session-path" => {
                if i + 1 >= args.len() {
                    return Err("--session-path requires a value".to_string());
                }
                session_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--max-tokens" => {
                if i + 1 >= args.len() {
                    return Err("--max-tokens requires a value".to_string());
                }
                max_tokens = args[i + 1]
                    .parse()
                    .map_err(|_| "invalid --max-tokens value")?;
                i += 2;
            }
            "-f" | "--format" => {
                if i + 1 >= args.len() {
                    return Err("--format requires a value".to_string());
                }
                format = args[i + 1].clone();
                i += 2;
            }
            "-q" | "--quiet" => {
                quiet = true;
                i += 1;
            }
            "-s" | "--save" => {
                save = true;
                i += 1;
            }
            "--no-save" => {
                no_save = true;
                i += 1;
            }
            "--detailed" => {
                detailed = true;
                i += 1;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}"));
            }
            _ => {
                return Err(format!("unexpected positional argument: {token}"));
            }
        }
    }

    if send
        && agent_name
            .as_ref()
            .map(|s| s.trim())
            .unwrap_or_default()
            .is_empty()
    {
        return Err("--send requires --agent <agent_name>".to_string());
    }

    Ok(ParsedCommand::CtxTransfer(ParsedCtxTransfer {
        project,
        last,
        source_provider,
        agent_name,
        send,
        dry_run,
        output,
        session_path,
        max_tokens,
        format,
        quiet,
        save,
        no_save,
        detailed,
    }))
}

fn parse_start(tokens: &[&String], project: Option<String>) -> Result<ParsedCommand, String> {
    let mut auto_permission = true;
    let mut reset_context = false;
    let mut agent_names: Vec<String> = Vec::new();

    for token in tokens {
        match token.as_str() {
            "start" => {}
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
    println!("ccbr {} - CCBR multi-agent CLI workspace", VERSION);
    println!();
    println!("Usage: ccbr [OPTIONS] [COMMAND] [ARGS...]");
    println!();
    println!("Options:");
    println!("  -h, --help                 Print this help message");
    println!("  -v, --version              Print version information");
    println!("      --project <PATH>       Use the CCBR project at <PATH>");
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
    println!("  shutdown                   Shut down the CCBR daemon");
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
    println!("  tools <doctor|install>     Tool management");
    println!("  roles <list|add|update>    Role management");
    println!("  fault <list|arm|clear>     Fault injection");
    println!("  repair <ack|retry|resubmit> Repair commands");
    println!("  autonew <provider>         Send /new to a provider pane");
    println!("  ctx-transfer [OPTIONS]     Transfer conversation context");
    println!("  version                    Show version");
    println!("  update                     Update CCBR");
    println!("  uninstall                  Uninstall CCBR");
    println!("  reinstall                  Reinstall CCBR");
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
    fn test_parse_explicit_start_command_skips_keyword() {
        let args = vec![
            "start".to_string(),
            "agent1".to_string(),
            "agent2".to_string(),
        ];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Start(s) = cmd {
            assert_eq!(s.agent_names, vec!["agent1", "agent2"]);
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

    #[test]
    fn test_parse_autonew() {
        let args = vec!["autonew".to_string(), "claude".to_string()];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::Autonew(a) = cmd {
            assert_eq!(a.provider, "claude");
        } else {
            panic!("expected Autonew");
        }
    }

    #[test]
    fn test_parse_autonew_help() {
        let args = vec!["autonew".to_string(), "--help".to_string()];
        let cmd = parse_args(&args).unwrap();
        assert!(matches!(cmd, ParsedCommand::Autonew(_)));
    }

    #[test]
    fn test_parse_ctx_transfer_defaults() {
        let args = vec!["ctx-transfer".to_string()];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::CtxTransfer(c) = cmd {
            assert_eq!(c.last, 3);
            assert_eq!(c.source_provider, "auto");
            assert_eq!(c.format, "markdown");
            assert_eq!(c.max_tokens, 8000);
            assert!(!c.send);
        } else {
            panic!("expected CtxTransfer");
        }
    }

    #[test]
    fn test_parse_ctx_transfer_options() {
        let args = vec![
            "ctx-transfer".to_string(),
            "--last".to_string(),
            "5".to_string(),
            "--from".to_string(),
            "claude".to_string(),
            "--send".to_string(),
            "--agent".to_string(),
            "gemini".to_string(),
            "--dry-run".to_string(),
        ];
        let cmd = parse_args(&args).unwrap();
        if let ParsedCommand::CtxTransfer(c) = cmd {
            assert_eq!(c.last, 5);
            assert_eq!(c.source_provider, "claude");
            assert!(c.send);
            assert_eq!(c.agent_name, Some("gemini".to_string()));
            assert!(c.dry_run);
        } else {
            panic!("expected CtxTransfer");
        }
    }

    #[test]
    fn test_parse_ctx_transfer_send_requires_agent() {
        let args = vec!["ctx-transfer".to_string(), "--send".to_string()];
        assert!(parse_args(&args).is_err());
    }
}
