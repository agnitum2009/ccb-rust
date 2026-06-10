from __future__ import annotations

import argparse
from collections.abc import Callable, Sequence
from textwrap import dedent


AuxiliaryHandler = Callable[[Sequence[str]], int]
ManagementHandler = Callable[[argparse.Namespace], int]

_MANAGEMENT_COMMANDS = {"update", "version", "uninstall", "reinstall"}


def dispatch_auxiliary_command(
    argv: Sequence[str],
    *,
    droid_handler: AuxiliaryHandler,
) -> int | None:
    tokens = list(argv)
    if tokens and tokens[0] == "droid" and len(tokens) > 1 and tokens[1] in {"setup-delegation", "test-delegation"}:
        return droid_handler(tokens[1:])
    return None


def dispatch_management_command(
    argv: Sequence[str],
    *,
    update_handler: ManagementHandler,
    version_handler: ManagementHandler,
    uninstall_handler: ManagementHandler,
    reinstall_handler: ManagementHandler,
) -> int | None:
    tokens = list(argv)
    if not tokens or tokens[0] not in _MANAGEMENT_COMMANDS:
        return None

    parser = _build_management_parser()
    args = parser.parse_args(tokens)
    if args.command == "update":
        return update_handler(args)
    if args.command == "version":
        return version_handler(args)
    if args.command == "uninstall":
        return uninstall_handler(args)
    if args.command == "reinstall":
        return reinstall_handler(args)
    parser.print_help()
    return 1


def parse_start_args(argv: Sequence[str]) -> argparse.Namespace:
    return build_start_parser().parse_args(list(argv))


def print_start_help(*, file=None) -> None:
    print(
        dedent(
            """
            usage: ccb [-s] [-n]

            Primary workflow:
              ccb                  Start project agents from `.ccb/ccb.config`.
              ccb -s               Safe start. Disable CLI auto-permission override.
              ccb -n               Rebuild runtime state while preserving config and managed agent history.
              ccb clear [agent...]  Send provider-native /clear to managed agent panes.
              ccb restart <agent> Restart one idle configured agent pane through ccbd.
              ccb reload            Apply a safe additive config reload, or reject with diagnostics.
              ccb reload --dry-run  Validate and plan config reload without mutation.
              ccb kill             Stop the current project's background runtime.
              ccb kill -f          Force cleanup project-owned runtime residue.
              ccb cleanup          Prune safe provider rebuildable caches after ccbd is stopped.

            Core commands:
              ccb ask <agent> [from <sender>] <message>
              ccb doctor

            Diagnostics-only control-plane status:
              ccb ping <agent|ccbd>

            Diagnostics-only observer:
              ccb pend <agent|job_id> [N]
              ccb pend --watch <agent|job_id>
              ccb pend --inbox [--detail] <agent>
              ccb pend --queue [--detail] <agent|all>

            Advanced views:
              ccb queue [--detail] <agent|all>
              ccb trace <id>

            Advanced recovery:
              ccb repair <ack|retry|resubmit> ...

            Management:
              ccb version | ccb update | ccb uninstall | ccb reinstall

            Tools:
              ccb tools doctor neovim
              ccb tools install neovim

            Roles:
              ccb roles list
              ccb roles install agentroles.ccb_self
              ccb roles update agentroles.ccb_self
              ccb roles add agentroles.ccb_self:codex
              ccb roles install agentroles.archi
              ccb roles update agentroles.archi
              ccb roles sync [path]
              ccb roles add agentroles.archi:codex
              ccb roles doctor agentroles.archi
            """
        ).strip(),
        file=file,
    )


def print_kill_help(*, file=None) -> None:
    print(
        dedent(
            """
            usage: ccb kill [-f]

            Project runtime cleanup:
              ccb kill     Stop the current project's ccbd, agents, and tmux namespace.
              ccb kill -f  Force cleanup project-owned runtime residue before `ccb -n`.

            Notes:
              - `kill` is project-scoped. It does not bootstrap a missing `.ccb`.
              - `kill` still works when `.ccb` exists but `ccb.config` is missing or stale.
              - Use `ccb -n` after `ccb kill` when you want to rebuild runtime state but keep config and managed agent history.
            """
        ).strip(),
        file=file,
    )


def print_command_help(command_name: str, *, file=None) -> bool:
    text = _COMMAND_HELP.get(command_name)
    if text is None:
        return False
    print(dedent(text).strip(), file=file)
    return True


_COMMAND_HELP = {
    "ping": """
        usage: ccb ping <agent|all|ccbd>

        Diagnostics-only control-plane status:
          ccb ping <agent>   Show cached runtime status for one named agent.
          ccb ping all       Show cached mounted-agent status across the project.
          ccb ping ccbd      Show cached project daemon status.
    """,
    "pend": """
        usage: ccb pend [--watch|--inbox|--queue] [--detail] <agent|job_id|all> [N]

        Diagnostics-only weak observer surface:
          These commands are not part of normal ask workflows.
          Primary weak observer entrypoint:
            ccb pend <agent>                    Show a non-authoritative observer snapshot for one agent.
            ccb pend <job_id>                   Show a non-authoritative observer snapshot for one submitted job.
            ccb pend --watch <agent|job_id>     Stream non-authoritative observer events via the converged observer entrypoint.
            ccb pend --inbox <agent>            Show a non-authoritative inbox summary via the converged observer entrypoint.
            ccb pend --inbox --detail <agent>   Expand inbox-item detail via the converged observer entrypoint.
            ccb pend --queue <agent|all>        Show the same non-authoritative backlog summary exposed by `ccb queue`.
            ccb pend --queue --detail <agent>   Expand queued-event detail through the observer entrypoint.
            ccb pend <target> N                 Show the latest N observer snapshot items.
          Use `ccb trace <id>` for lineage when needed.
    """,
    "watch": """
        usage: ccb watch <agent|job_id>

        Diagnostics-only weak observer compatibility entrypoint:
          ccb watch <agent>   Stream non-authoritative observer events for one agent.
          ccb watch <job_id>  Stream non-authoritative observer events for one job until terminal completion or timeout.
          This is not part of normal ask workflows.
          Prefer `ccb pend --watch <agent|job_id>` as the converged observer entrypoint.
          Do not treat non-terminal watch output as authoritative completion.
          Use `ccb trace <id>` for lineage when needed.
    """,
    "queue": """
        usage: ccb queue [--detail] <agent_name|all>

        Advanced backlog view:
          ccb queue <agent_name>            Show a non-authoritative observer summary for one agent.
          ccb queue --detail <agent_name>   Expand queued-event details for one agent.
          ccb queue all                     Show non-authoritative observer backlog state across the project.
          `ccb pend --queue [--detail] <agent|all>` remains the equivalent weak-observer form.
          Use `ccb trace <id>` for lineage when needed.
    """,
    "trace": """
        usage: ccb trace <submission_id|message_id|attempt_id|reply_id|job_id>

        Advanced lineage view:
          ccb trace <id>   Show the full job/message/reply lineage for one id.
    """,
    "inbox": """
        usage: ccb inbox [--detail] <agent_name>

        Weak observer compatibility entrypoint:
          ccb inbox <agent_name>            Show a non-authoritative observer summary for one agent.
          ccb inbox --detail <agent_name>   Expand inbox-item detail for one agent.
          Prefer `ccb pend --inbox [--detail] <agent>` as the converged observer entrypoint.
          Use `ccb trace <id>` for lineage when needed.
    """,
    "logs": """
        usage: ccb logs <agent>

        Runtime diagnostics compatibility view:
          ccb logs <agent>   Tail the current runtime/session log for one agent.
          Prefer `ccb doctor logs <agent>` as the converged diagnostics entrypoint.
    """,
    "doctor-logs": """
        usage: ccb doctor logs <agent>

        Runtime log diagnostics subview:
          ccb doctor logs <agent>   Tail the current runtime/session log for one agent through the primary diagnostics entrypoint.
          `ccb logs <agent>` remains a compatibility alias.
    """,
    "ps": """
        usage: ccb ps

        Runtime diagnostics compatibility view:
          ccb ps   Show known runtime/session/workspace bindings.
          Prefer `ccb doctor ps` as the converged diagnostics entrypoint.
    """,
    "doctor-ps": """
        usage: ccb doctor ps

        Runtime diagnostics subview:
          ccb doctor ps   Show known runtime/session/workspace bindings through the primary diagnostics entrypoint.
          `ccb ps` remains a compatibility alias.
    """,
    "doctor-storage": """
        usage: ccb doctor storage [--json]

        Storage diagnostics subview:
          ccb doctor storage        Show .ccb storage class totals and largest entries.
          ccb doctor storage --json Emit full storage classification payload.
    """,
    "cleanup": """
        usage: ccb cleanup

        Storage cleanup:
          ccb cleanup   Prune safe provider rebuildable caches after ccbd is stopped.

        Safety:
          - Refuses to run while ccbd is active or ask jobs are pending/running.
          - Keeps Claude versions currently referenced by managed homes.
          - Does not remove provider sessions, auth, plugin bundles, mailbox data, or runtime authority.
          - Use `ccb doctor storage` before cleanup to inspect storage classes.
    """,
    "clear": """
        usage: ccb clear [agent_name|all]...

        Agent context reset:
          ccb clear             Send /clear to every configured mounted agent pane.
          ccb clear agent1      Send /clear to one agent pane.
          ccb clear agent1 agent2
                                Send /clear to multiple agent panes.

        Notes:
          - This sends the provider-native /clear command into each pane.
          - It does not delete .ccb state, workspaces, auth, sessions, or logs.
          - Use `ccb kill` or the sidebar restart control when you need process restart.
    """,
    "restart": """
        usage: ccb restart <agent_name>

        Guarded single-agent runtime restart:
          ccb restart agent1   Restart one configured mounted agent pane through ccbd.

        Safety:
          - Target authority comes from the current mounted daemon graph.
          - Refuses when the agent is busy, queued, delivering a reply, or waiting on callback continuation.
          - Does not support `restart all`, window-level restart, or raw tmux mutation.
    """,
    "doctor": """
        usage: ccb doctor [ps|logs <agent>|storage] [--output [PATH]]

        Deep diagnostics:
          ccb doctor               Print project diagnostic summary.
          ccb doctor ps            Show the runtime/session/workspace diagnostics subview.
          ccb doctor logs <agent>  Tail the runtime/session log diagnostics subview for one agent.
          ccb doctor storage       Show .ccb storage class totals.
          ccb doctor --output      Export a support bundle to the default path.
          ccb doctor --output PATH Export a support bundle to PATH.
          `ccb ps` and `ccb logs <agent>` remain compatibility entrypoints.
    """,
    "cancel": """
        usage: ccb cancel <job_id>

        Job control view:
          ccb cancel <job_id>   Request cancellation for one submitted job.
    """,
    "ack": """
        usage: ccb ack <agent_name> [inbound_event_id]

        Advanced recovery compatibility entrypoint:
          ccb ack <agent_name> [inbound_event_id]   Acknowledge reply/inbox progress for one agent.
          Prefer `ccb repair ack <agent_name> [inbound_event_id]` as the converged recovery entrypoint.
    """,
    "repair-ack": """
        usage: ccb repair ack <agent_name> [inbound_event_id]

        Advanced recovery subcommand:
          ccb repair ack <agent_name> [inbound_event_id]   Acknowledge reply/inbox progress for one agent.
          `ccb ack <agent_name> [inbound_event_id]` remains a compatibility alias.
    """,
    "retry": """
        usage: ccb retry <job_id|attempt_id>

        Advanced recovery compatibility entrypoint:
          ccb retry <job_id|attempt_id>   Retry one failed or incomplete job/attempt lineage.
          Prefer `ccb repair retry <job_id|attempt_id>` as the converged recovery entrypoint.
    """,
    "repair-retry": """
        usage: ccb repair retry <job_id|attempt_id>

        Advanced recovery subcommand:
          ccb repair retry <job_id|attempt_id>   Retry one failed or incomplete job/attempt lineage.
          `ccb retry <job_id|attempt_id>` remains a compatibility alias.
    """,
    "resubmit": """
        usage: ccb resubmit <message_id>

        Advanced recovery compatibility entrypoint:
          ccb resubmit <message_id>   Create a fresh submission from one prior message lineage.
          Prefer `ccb repair resubmit <message_id>` as the converged recovery entrypoint.
    """,
    "repair-resubmit": """
        usage: ccb repair resubmit <message_id>

        Advanced recovery subcommand:
          ccb repair resubmit <message_id>   Create a fresh submission from one prior message lineage.
          `ccb resubmit <message_id>` remains a compatibility alias.
    """,
    "repair": """
        usage: ccb repair <ack|retry|resubmit> ...

        Advanced recovery:
          ccb repair ack <agent_name> [inbound_event_id]   Acknowledge reply/inbox progress for one agent.
          ccb repair retry <job_id|attempt_id>             Retry one failed or incomplete job/attempt lineage.
          ccb repair resubmit <message_id>                 Create a fresh submission from one prior message lineage.
          Legacy `ack` / `retry` / `resubmit` commands remain compatibility entrypoints.
    """,
    "config": """
        usage: ccb config validate

        Config validation:
          ccb config validate   Validate `.ccb/ccb.config` for the current project.
    """,
    "reload": """
        usage: ccb reload [--dry-run]

        Reload:
          ccb reload             Apply safe explicit changes: view-only, append-only add_agent/add_window, or idle remove_agent.
          ccb reload --dry-run   Ask the mounted daemon to validate `.ccb/ccb.config` and return a no-mutation reload plan.

        Explicit reload boundary:
          - Busy remove_agent, replace_agent, move_agent, and arbitrary layout changes are rejected.
          - No config watch is started; replace and full kill/reflow of existing panes are not implemented.
          - Non-dry-run output includes stage, plan_class, graph version, diagnostics, and any residue.
    """,
    "tools": """
        usage: ccb tools <doctor|install|update> neovim

        Managed tool provisioning:
          ccb tools doctor neovim   Inspect the CCB-managed Neovim/LazyVim profile.
          ccb tools install neovim  Prepare isolated ccb-nvim wrapper/profile.
          ccb tools update neovim   Refresh the managed profile wrapper.
    """,
    "roles": """
        usage: ccb roles <list|show|install|update|sync|add|doctor> ...

        Role Pack management:
          ccb roles list
          ccb roles show agentroles.ccb_self
          ccb roles install agentroles.ccb_self
          ccb roles update agentroles.ccb_self
          ccb roles add agentroles.ccb_self:codex
          ccb roles show agentroles.archi
          ccb roles install agentroles.archi
          ccb roles update agentroles.archi
          ccb roles sync [path]
          ccb roles add agentroles.archi:codex
          ccb roles doctor agentroles.archi
    """,
}


def _build_management_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="ccb", description="Claude AI unified launcher", add_help=True)
    subparsers = parser.add_subparsers(dest="command", help="Subcommands")

    update_parser = subparsers.add_parser("update", help="Update to latest or specified version")
    update_parser.add_argument("target", nargs="?", help="version like '4', '4.1', '4.1.3'")

    subparsers.add_parser("version", help="Show version and check for updates")
    subparsers.add_parser("uninstall", help="Uninstall ccb and clean configs")
    subparsers.add_parser("reinstall", help="Reinstall ccb and refresh configs")
    return parser


def build_start_parser() -> argparse.ArgumentParser:
    start_parser = argparse.ArgumentParser(
        prog="ccb",
        description="Claude AI unified launcher",
        add_help=False,
    )
    start_parser.add_argument("-s", "--safe", action="store_true", default=False, help=argparse.SUPPRESS)
    start_parser.add_argument(
        "-n",
        "--new-context",
        dest="new_context",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    return start_parser
