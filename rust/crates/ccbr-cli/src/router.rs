//! Mirrors Python `lib/cli/router.py`.
//!
//! CLI command routing: dispatch auxiliary/management commands, help text, start/kill parsers.
//! 1:1 alignment with Python module.

use std::collections::HashSet;
use std::io::Write;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Handler for auxiliary subcommands (receives remaining argv, returns exit code).
pub type AuxiliaryHandler = Box<dyn Fn(&[String]) -> i32>;

/// Handler for management subcommands (receives parsed args, returns exit code).
pub type ManagementHandler = Box<dyn Fn(&ManagementArgs) -> i32>;

// ---------------------------------------------------------------------------
// Management parsed args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ManagementArgs {
    pub command: String,
    pub target: Option<String>,
}

// ---------------------------------------------------------------------------
// Start parsed args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct StartArgs {
    pub safe: bool,
    pub new_context: bool,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

static MANAGEMENT_COMMANDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["update", "version", "uninstall", "reinstall"]));

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn dispatch_auxiliary_command(
    argv: &[String],
    droid_handler: &AuxiliaryHandler,
) -> Option<i32> {
    if argv.len() >= 2
        && argv[0] == "droid"
        && (argv[1] == "setup-delegation" || argv[1] == "test-delegation")
    {
        return Some(droid_handler(&argv[1..]));
    }
    None
}

pub fn dispatch_management_command(
    argv: &[String],
    update_handler: &ManagementHandler,
    version_handler: &ManagementHandler,
    uninstall_handler: &ManagementHandler,
    reinstall_handler: &ManagementHandler,
) -> Option<i32> {
    if argv.is_empty() || !MANAGEMENT_COMMANDS.contains(argv[0].as_str()) {
        return None;
    }

    let args = parse_management_args(argv);
    match args.command.as_str() {
        "update" => Some(update_handler(&args)),
        "version" => Some(version_handler(&args)),
        "uninstall" => Some(uninstall_handler(&args)),
        "reinstall" => Some(reinstall_handler(&args)),
        _ => Some(1),
    }
}

fn parse_management_args(argv: &[String]) -> ManagementArgs {
    let command = argv.first().cloned().unwrap_or_default();
    let target = if command == "update" {
        argv.get(1).cloned()
    } else {
        None
    };
    ManagementArgs { command, target }
}

// ---------------------------------------------------------------------------
// Start parser
// ---------------------------------------------------------------------------

pub fn parse_start_args(argv: &[String]) -> StartArgs {
    let mut safe = false;
    let mut new_context = false;
    for arg in argv {
        match arg.as_str() {
            "-s" | "--safe" => safe = true,
            "-n" | "--new-context" => new_context = true,
            _ => {}
        }
    }
    StartArgs { safe, new_context }
}

#[derive(Debug, Clone)]
pub struct StartParserSpec {
    pub prog: String,
    pub description: String,
}

pub fn build_start_parser() -> StartParserSpec {
    StartParserSpec {
        prog: "ccbr".to_string(),
        description: "Claude AI unified launcher".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Help text
// ---------------------------------------------------------------------------

pub fn print_start_help<W: Write>(out: &mut W) -> std::io::Result<()> {
    write!(
        out,
        "\
usage: ccbr [-s] [-n]

Primary workflow:
  ccbr                  Start project agents from `.ccbr/ccbr.config`.
  ccbr -s               Safe start. Disable CLI auto-permission override.
  ccbr -n               Rebuild runtime state while preserving config and managed agent history.
  ccbr clear [agent...]  Send provider-native /clear to managed agent panes.
  ccbr restart <agent> Restart one idle configured agent pane through ccbrd.
  ccbr reload            Apply a safe additive config reload, or reject with diagnostics.
  ccbr reload --dry-run  Validate and plan config reload without mutation.
  ccbr maintenance status Show maintenance heartbeat config and stored status.
  ccbr maintenance tick   Run one maintenance heartbeat diagnosis tick.
  ccbr kill             Stop the current project's background runtime.
  ccbr kill -f          Force cleanup project-owned runtime residue.
  ccbr cleanup          Prune safe provider rebuildable caches after ccbrd is stopped.

Core commands:
  ccbr ask <agent> [from <sender>] <message>
  ccbr doctor

Diagnostics-only control-plane status:
  ccbr ping <agent|ccbrd>

Diagnostics-only observer:
  ccbr pend <agent|job_id> [N]
  ccbr pend --watch <agent|job_id>
  ccbr pend --inbox [--detail] <agent>
  ccbr pend --queue [--detail] <agent|all>

Advanced views:
  ccbr queue [--detail] <agent|all>
  ccbr trace <id>

Advanced recovery:
  ccbr repair <ack|retry|resubmit> ...

Management:
  ccbr version | ccbr update | ccbr uninstall | ccbr reinstall

Tools:
  ccbr tools doctor neovim
  ccbr tools install neovim

Roles:
  ccbr roles list
  ccbr roles install agentroles.ccbr_self
  ccbr roles update agentroles.ccbr_self
  ccbr roles add agentroles.ccbr_self:codex
  ccbr roles install agentroles.archi
  ccbr roles update agentroles.archi
  ccbr roles sync [path]
  ccbr roles add agentroles.archi:codex
  ccbr roles doctor agentroles.archi\n"
    )
}

pub fn print_kill_help<W: Write>(out: &mut W) -> std::io::Result<()> {
    write!(out, "\
usage: ccbr kill [-f]

Project runtime cleanup:
  ccbr kill     Stop the current project's ccbrd, agents, and tmux namespace.
  ccbr kill -f  Force cleanup project-owned runtime residue before `ccbr -n`.

Notes:
  - `kill` is project-scoped. It does not bootstrap a missing `.ccbr`.
  - `kill` still works when `.ccbr` exists but `ccbr.config` is missing or stale.
  - Use `ccbr -n` after `ccbr kill` when you want to rebuild runtime state but keep config and managed agent history.\n")
}

pub fn print_command_help<W: Write>(out: &mut W, command_name: &str) -> std::io::Result<bool> {
    if let Some(text) = command_help_text(command_name) {
        write!(out, "{}", text)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn command_help_text(command_name: &str) -> Option<&'static str> {
    COMMAND_HELP
        .iter()
        .find(|(k, _)| *k == command_name)
        .map(|(_, v)| *v)
}

static COMMAND_HELP: &[(&str, &str)] = &[
    ("ping", "\
usage: ccbr ping <agent|all|ccbrd>

Diagnostics-only control-plane status:
  ccbr ping <agent>   Show cached runtime status for one named agent.
  ccbr ping all       Show cached mounted-agent status across the project.
  ccbr ping ccbrd      Show cached project daemon status.
"),
    ("pend", "\
usage: ccbr pend [--watch|--inbox|--queue] [--detail] <agent|job_id|all> [N]

Diagnostics-only weak observer surface:
  These commands are not part of normal ask workflows.
  Primary weak observer entrypoint:
    ccbr pend <agent>                    Show a non-authoritative observer snapshot for one agent.
    ccbr pend <job_id>                   Show a non-authoritative observer snapshot for one submitted job.
    ccbr pend --watch <agent|job_id>     Stream non-authoritative observer events via the converged observer entrypoint.
    ccbr pend --inbox <agent>            Show a non-authoritative inbox summary via the converged observer entrypoint.
    ccbr pend --inbox --detail <agent>   Expand inbox-item detail via the converged observer entrypoint.
    ccbr pend --queue <agent|all>        Show the same non-authoritative backlog summary exposed by `ccbr queue`.
    ccbr pend --queue --detail <agent>   Expand queued-event detail through the observer entrypoint.
    ccbr pend <target> N                 Show the latest N observer snapshot items.
  Use `ccbr trace <id>` for lineage when needed.
"),
    ("watch", "\
usage: ccbr watch <agent|job_id>

Diagnostics-only weak observer compatibility entrypoint:
  ccbr watch <agent>   Stream non-authoritative observer events for one agent.
  ccbr watch <job_id>  Stream non-authoritative observer events for one job until terminal completion or timeout.
  This is not part of normal ask workflows.
  Prefer `ccbr pend --watch <agent|job_id>` as the converged observer entrypoint.
  Do not treat non-terminal watch output as authoritative completion.
  Use `ccbr trace <id>` for lineage when needed.
"),
    ("queue", "\
usage: ccbr queue [--detail] <agent_name|all>

Advanced backlog view:
  ccbr queue <agent_name>            Show a non-authoritative observer summary for one agent.
  ccbr queue --detail <agent_name>   Expand queued-event details for one agent.
  ccbr queue all                     Show non-authoritative observer backlog state across the project.
  `ccbr pend --queue [--detail] <agent|all>` remains the equivalent weak-observer form.
  Use `ccbr trace <id>` for lineage when needed.
"),
    ("trace", "\
usage: ccbr trace <submission_id|message_id|attempt_id|reply_id|job_id>

Advanced lineage view:
  ccbr trace <id>   Show the full job/message/reply lineage for one id.
"),
    ("inbox", "\
usage: ccbr inbox [--detail] <agent_name>

Weak observer compatibility entrypoint:
  ccbr inbox <agent_name>            Show a non-authoritative observer summary for one agent.
  ccbr inbox --detail <agent_name>   Expand inbox-item detail for one agent.
  Prefer `ccbr pend --inbox [--detail] <agent>` as the converged observer entrypoint.
  Use `ccbr trace <id>` for lineage when needed.
"),
    ("logs", "\
usage: ccbr logs <agent>

Runtime diagnostics compatibility view:
  ccbr logs <agent>   Tail the current runtime/session log for one agent.
  Prefer `ccbr doctor logs <agent>` as the converged diagnostics entrypoint.
"),
    ("doctor-logs", "\
usage: ccbr doctor logs <agent>

Runtime log diagnostics subview:
  ccbr doctor logs <agent>   Tail the current runtime/session log for one agent through the primary diagnostics entrypoint.
  `ccbr logs <agent>` remains a compatibility alias.
"),
    ("ps", "\
usage: ccbr ps

Runtime diagnostics compatibility view:
  ccbr ps   Show known runtime/session/workspace bindings.
  Prefer `ccbr doctor ps` as the converged diagnostics entrypoint.
"),
    ("doctor-ps", "\
usage: ccbr doctor ps

Runtime diagnostics subview:
  ccbr doctor ps   Show known runtime/session/workspace bindings through the primary diagnostics entrypoint.
  `ccbr ps` remains a compatibility alias.
"),
    ("doctor-storage", "\
usage: ccbr doctor storage [--json]

Storage diagnostics subview:
  ccbr doctor storage        Show .ccbr storage class totals and largest entries.
  ccbr doctor storage --json Emit full storage classification payload.
"),
    ("cleanup", "\
usage: ccbr cleanup

Storage cleanup:
  ccbr cleanup   Prune safe provider rebuildable caches after ccbrd is stopped.

Safety:
  - Refuses to run while ccbrd is active or ask jobs are pending/running.
  - Keeps Claude versions currently referenced by managed homes.
  - Does not remove provider sessions, auth, plugin bundles, mailbox data, or runtime authority.
  - Use `ccbr doctor storage` before cleanup to inspect storage classes.
"),
    ("clear", "\
usage: ccbr clear [agent_name|all]...

Agent context reset:
  ccbr clear             Send /clear to every configured mounted agent pane.
  ccbr clear agent1      Send /clear to one agent pane.
  ccbr clear agent1 agent2
                        Send /clear to multiple agent panes.

Notes:
  - This sends the provider-native /clear command into each pane.
  - It does not delete .ccbr state, workspaces, auth, sessions, or logs.
  - Use `ccbr kill` or the sidebar restart control when you need process restart.
"),
    ("restart", "\
usage: ccbr restart <agent_name>

Guarded single-agent runtime restart:
  ccbr restart agent1   Restart one configured mounted agent pane through ccbrd.

Safety:
  - Target authority comes from the current mounted daemon graph.
  - Refuses when the agent is busy, queued, delivering a reply, or waiting on callback continuation.
  - Does not support `restart all`, window-level restart, or raw tmux mutation.
"),
    ("maintenance", "\
usage: ccbr maintenance <status|tick|schedule>

Maintenance heartbeat diagnostics:
  ccbr maintenance status   Show configured heartbeat policy plus stored schedule/status state.
  ccbr maintenance tick     Run one diagnosis tick, update heartbeat status/schedule when enabled.
  ccbr maintenance schedule --after 5m [--reason TEXT]
                           Schedule the next heartbeat follow-up.

Safety:
  - tick reads ccbrd/project-view evidence and may write only maintenance-heartbeat status/schedule/activation records.
  - non-healthy tick may submit one silent ask to the configured assessor, default ccbr_self.
  - tick does not run repairs or start providers.
  - runner is an internal project-scoped schedule consumer used by startup ensure.
  - enable and disable are config-authority in v1; edit [maintenance.heartbeat].enabled.
  - Status reads `.ccbr/ccbrd/maintenance-heartbeat/`, not `.ccbr/ccbrd/heartbeats/`.
"),
    ("doctor", "\
usage: ccbr doctor [ps|logs <agent>|storage] [--output [PATH]]

Deep diagnostics:
  ccbr doctor               Print project diagnostic summary.
  ccbr doctor ps            Show the runtime/session/workspace diagnostics subview.
  ccbr doctor logs <agent>  Tail the runtime/session log diagnostics subview for one agent.
  ccbr doctor storage       Show .ccbr storage class totals.
  ccbr doctor --output      Export a support bundle to the default path.
  ccbr doctor --output PATH Export a support bundle to PATH.
  `ccbr ps` and `ccbr logs <agent>` remain compatibility entrypoints.
"),
    ("cancel", "\
usage: ccbr cancel <job_id>

Job control view:
  ccbr cancel <job_id>   Request cancellation for one submitted job.
"),
    ("ack", "\
usage: ccbr ack <agent_name> [inbound_event_id]

Advanced recovery compatibility entrypoint:
  ccbr ack <agent_name> [inbound_event_id]   Acknowledge reply/inbox progress for one agent.
  Prefer `ccbr repair ack <agent_name> [inbound_event_id]` as the converged recovery entrypoint.
"),
    ("repair-ack", "\
usage: ccbr repair ack <agent_name> [inbound_event_id]

Advanced recovery subcommand:
  ccbr repair ack <agent_name> [inbound_event_id]   Acknowledge reply/inbox progress for one agent.
  `ccbr ack <agent_name> [inbound_event_id]` remains a compatibility alias.
"),
    ("retry", "\
usage: ccbr retry <job_id|attempt_id>

Advanced recovery compatibility entrypoint:
  ccbr retry <job_id|attempt_id>   Retry one failed or incomplete job/attempt lineage.
  Prefer `ccbr repair retry <job_id|attempt_id>` as the converged recovery entrypoint.
"),
    ("repair-retry", "\
usage: ccbr repair retry <job_id|attempt_id>

Advanced recovery subcommand:
  ccbr repair retry <job_id|attempt_id>   Retry one failed or incomplete job/attempt lineage.
  `ccbr retry <job_id|attempt_id>` remains a compatibility alias.
"),
    ("resubmit", "\
usage: ccbr resubmit <message_id>

Advanced recovery compatibility entrypoint:
  ccbr resubmit <message_id>   Create a fresh submission from one prior message lineage.
  Prefer `ccbr repair resubmit <message_id>` as the converged recovery entrypoint.
"),
    ("repair-resubmit", "\
usage: ccbr repair resubmit <message_id>

Advanced recovery subcommand:
  ccbr repair resubmit <message_id>   Create a fresh submission from one prior message lineage.
  `ccbr resubmit <message_id>` remains a compatibility alias.
"),
    ("repair", "\
usage: ccbr repair <ack|retry|resubmit> ...

Advanced recovery:
  ccbr repair ack <agent_name> [inbound_event_id]   Acknowledge reply/inbox progress for one agent.
  ccbr repair retry <job_id|attempt_id>             Retry one failed or incomplete job/attempt lineage.
  ccbr repair resubmit <message_id>                 Create a fresh submission from one prior message lineage.
  Legacy `ack` / `retry` / `resubmit` commands remain compatibility entrypoints.
"),
    ("config", "\
usage: ccbr config validate

Config validation:
  ccbr config validate   Validate `.ccbr/ccbr.config` for the current project.
"),
    ("reload", "\
usage: ccbr reload [--dry-run]

Reload:
  ccbr reload             Apply safe explicit changes: view-only, append-only add_agent/add_window, or idle remove_agent.
  ccbr reload --dry-run   Ask the mounted daemon to validate `.ccbr/ccbr.config` and return a no-mutation reload plan.

Explicit reload boundary:
  - Busy remove_agent, replace_agent, move_agent, and arbitrary layout changes are rejected.
  - No config watch is started; replace and full kill/reflow of existing panes are not implemented.
  - Non-dry-run output includes stage, plan_class, graph version, diagnostics, and any residue.
"),
    ("tools", "\
usage: ccbr tools <doctor|install|update> neovim

Managed tool provisioning:
  ccbr tools doctor neovim   Inspect the CCBR-managed Neovim/LazyVim profile.
  ccbr tools install neovim  Prepare isolated ccbr-nvim wrapper/profile.
  ccbr tools update neovim   Refresh the managed profile wrapper.
"),
    ("roles", "\
usage: ccbr roles <list|show|install|update|sync|add|doctor> ...

Role Pack management:
  ccbr roles list
  ccbr roles show agentroles.ccbr_self
  ccbr roles install agentroles.ccbr_self
  ccbr roles update agentroles.ccbr_self
  ccbr roles add agentroles.ccbr_self:codex
  ccbr roles show agentroles.archi
  ccbr roles install agentroles.archi
  ccbr roles update agentroles.archi
  ccbr roles sync [path]
  ccbr roles add agentroles.archi:codex
  ccbr roles doctor agentroles.archi
"),
];
