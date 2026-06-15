# Command And Config Source Inventory

Date: 2026-06-10

## Status

First-pass source inventory for the user manual and the developer manual
configuration chapter. This is not yet a complete command reference.

## CLI Dispatch Shape

Primary files:

- `lib/cli/entrypoint_runtime.py`
- `lib/cli/router.py`
- `lib/cli/parser.py`
- `lib/cli/parser_runtime/constants.py`
- `lib/cli/parser_runtime/commands.py`
- `lib/cli/parser_runtime/fault.py`
- `lib/cli/ask_usage.py`

Observed responsibilities:

- `run_cli_entrypoint` handles `--print-version`, sidebar internal commands,
  background update refresh, post-update hooks, help rendering, version alias
  rewriting, removed command guidance, auxiliary commands, management commands,
  tools, roles, startup release update checks, and then phase2 dispatch.
- `SUBCOMMANDS` defines the ordinary phase2 command set:
  `ask`, `cancel`, `clear`, `cleanup`, `kill`, `ps`, `ping`, `watch`, `pend`,
  `queue`, `trace`, `resubmit`, `retry`, `wait-any`, `wait-all`,
  `wait-quorum`, `inbox`, `ack`, `logs`, `maintenance`, `doctor`, `repair`,
  `config`, `fault`, `reload`, and `restart`.
- `ask` has a separate usage surface and parser because it combines route
  parsing, sender inference, stdin body handling, artifact policy, callback
  policy, and legacy diagnostics aliases.
- Removed commands are still recognized for user guidance: `open`, `up`,
  `mail`, and `provider`.

Manual implication:

- The user manual command chapter should be organized by dispatch layer:
  startup/default invocation, ordinary phase2 commands, communication commands,
  diagnostics/maintenance commands, role/tool commands, management commands,
  and removed-command migration notes.
- The developer manual should explain that the visible CLI is not a single
  parser; early entrypoint routing owns several commands before phase2 sees
  tokens.

## Ordinary Runtime Commands

Primary file:

- `lib/cli/parser_runtime/commands.py`

Observed commands and parsing rules:

- `cancel <job_id>`.
- `clear [agent_names...]`; `all` maps to all configured agents and cannot be
  combined with explicit agent names.
- `restart <agent_name>`; `all` is rejected.
- `maintenance [status|tick|schedule|enable|disable]`; omitted action defaults
  to `status`.
- `kill [-f|--force]`.
- `cleanup`.
- `ps`.
- `ping <agent_name|all>`.
- `watch <agent_name|job_id>`.
- `pend [--watch|--inbox|--queue] [--detail] <target> [count]`; at most one
  observer mode, `--detail` requires inbox or queue mode, and `count` is only
  allowed in snapshot mode.
- `queue [--detail] <target>`.
- `trace <submission_id|message_id|attempt_id|reply_id|job_id>`.
- `resubmit <message_id>`.
- `repair <ack|retry|resubmit> ...`, delegated to the underlying parsers.
- `retry <job_id|attempt_id>`.
- `wait-any [--timeout N] <target>`, `wait-all [--timeout N] <target>`, and
  `wait-quorum [--timeout N] <quorum> <target>`.
- `inbox [--detail] <agent_name>`.
- `ack <agent_name> [inbound_event_id]`.
- `logs <agent_name>`.
- `doctor`, with `ps`/`--runtime`, `logs`/`--logs`, `storage [--json]`, and
  `--output [PATH]`; `--bundle` is intentionally rejected.
- `config validate`.
- `reload [--dry-run]`.

## Ask Command Surface

Primary files:

- `lib/cli/parser_runtime/ask.py`
- `lib/cli/ask_usage.py`
- `lib/cli/services/ask.py`
- `lib/cli/services/ask_runtime/submission.py`

Observed command surface:

- `ccb ask [--compact] [--silence] [--callback] [--artifact-request]
  [--artifact-reply] [--artifact-io] <target> [--] <message...>`.
- `--artifact-io` enables both explicit request and reply artifact behavior.
- Nested asks from active tasks must use `--callback` or `--silence`.
- `ask get <job_id>` and `ask cancel <job_id>` remain diagnostics-oriented
  aliases.

Manual implication:

- The user manual should describe ask flags as delivery, dependency, and
  artifact policies rather than as formatting options.
- The communication chapter should connect this surface to
  `MessageEnvelope.route_options` and dispatcher/message-bureau state.

## Role Commands

Primary files:

- `lib/cli/roles_runtime/commands.py`
- `lib/rolepacks/`
- `lib/agents/config_loader_runtime/role_lookup.py`

Observed commands:

- `roles list`.
- `roles show <role_id>`.
- `roles install [role_id] [--path PATH] [--skip-tools]`.
- `roles update [role_id] [--path PATH] [--skip-tools]`.
- `roles sync [path] [--with-tools]`.
- `roles doctor <role_id>`.
- `roles add <role_spec> [--agent NAME] [--provider PROVIDER] [--window WINDOW]`.

Observed role/config connection:

- `roles add` locates the nearest project `.ccb` anchor and calls
  `add_role_to_project_config`.
- `role_spec` uses the same layout leaf parsing path as config layout specs.
- Config loading recognizes role id shorthand in compact configs and topology
  windows, resolving installed roles to their default agent names.

Manual implication:

- The answer to "how an Archi role loads into CCB" belongs in both manuals:
  role commands install/update/sync rolepacks, `roles add` writes project
  config, and config loading expands role ids into agent specs before runtime
  mounting.

## Tool Commands

Primary file:

- `lib/cli/tools_runtime/neovim.py`

Observed commands:

- `tools doctor neovim`.
- `tools install neovim`.
- `tools update neovim`.

Observed behavior:

- Tool support is currently Neovim-specific.
- The managed tool stores state under XDG data/state/cache roots in
  `ccb/tools/neovim`.
- `CCB_INSTALL_NEOVIM`, `CCB_LAZYVIM_PROFILE`,
  `CCB_LAZYVIM_SYNC_TIMEOUT_S`, and `CCB_LAZYVIM_BOOTSTRAP_TIMEOUT_S`
  influence provisioning.

## Management And Auxiliary Commands

Primary files:

- `lib/cli/entrypoint_runtime.py`
- `lib/cli/router.py`
- `lib/cli/management.py`
- `lib/cli/auxiliary.py`

Observed command groups:

- Management: `version`, `update`, `uninstall`, `reinstall`.
- Version aliases: `-v` and `--version` rewrite to `version`; internal
  `--print-version` prints the source version directly.
- Auxiliary Droid integration: `droid setup-delegation` and
  `droid test-delegation`.

## Fault Injection

Primary file:

- `lib/cli/parser_runtime/fault.py`

Observed commands:

- `fault list`.
- `fault arm <agent_name> --task-id TASK --reason REASON --count N --error TEXT`.
- `fault clear <rule_id|all>`.

Manual implication:

- Fault injection should be in an advanced diagnostics/testing appendix, not
  the first user workflow chapter.

## Config Loading Source Map

Primary files:

- `lib/agents/config_loader.py`
- `lib/agents/config_loader_runtime/common.py`
- `lib/agents/config_loader_runtime/io_runtime/documents.py`
- `lib/agents/config_loader_runtime/parsing_runtime/validation.py`
- `lib/agents/config_loader_runtime/parsing_runtime/topology.py`
- `lib/agents/config_loader_runtime/parsing_runtime/agent_specs.py`
- `lib/agents/config_loader_runtime/parsing_runtime/provider_profiles.py`
- `lib/agents/config_loader_runtime/defaults_runtime/`

Observed source precedence:

- Project config: `.ccb/ccb.config`.
- User default config: the configured user default path from
  `user_default_config_path`.
- Built-in default config when neither exists.

Observed document shapes:

- Compact layout text, parsed from layout tokens such as
  `agent_name:provider` and reserved `cmd`.
- Rich TOML, parsed through `tomllib`, `tomli`, or `toml`.
- Hybrid compact-plus-TOML overlay. The overlay supports only `agents` and
  `maintenance`, cannot define agents outside the compact layout, and cannot
  redefine compact header-owned fields `provider` or `workspace_mode`.

Observed top-level config keys:

- `version`, `default_agents`, `agents`, `cmd_enabled`, `layout`, `ui`,
  `windows`, `tool_windows`, `entry_window`, and `maintenance`.

Observed topology rules:

- `version` must be `2`.
- Without `[windows]`, `default_agents` is required, `layout` and
  `cmd_enabled` are legacy-compatible fields, and `ui`/`entry_window` are
  rejected.
- With `[windows]`, `default_agents` is derived from window leaves,
  `default_agents`, `layout`, and `cmd_enabled` are rejected, and `tool_windows`
  plus `ui.sidebar` become available.
- Window leaves must declare providers, cannot use `cmd`, and cannot repeat
  agent names across windows.
- With `[windows]`, window leaves are the canonical source for provider and
  default `inplace`/`git-worktree` workspace mode. `[agents.<name>]` tables are
  overlays and should not repeat those topology-owned fields.
- Legacy rich TOML that repeats matching `provider` or default workspace mode
  still loads, but `ccb config validate` reports style warnings.
- `[agents.<name>]` tables not referenced by `[windows]` are ignored as stale
  overlays and reported as style warnings.
- `tool_windows` entries require `command`; `label` and `show_in_sidebar` are
  optional.
- `ui.sidebar` supports `mode`, `width`, `bottom_height`, and `view`.
- `ui.sidebar.view` supports `agents_height`, `comms_height`, `tips_height`,
  `comms_limit`, `comms_compact`, `tips_enabled`, and `tips`.

Observed agent keys:

- `provider`, `target`, `workspace_mode`, `workspace_root`, `workspace_path`,
  `workspace_group`, `provider_command_template`, `runtime_mode`, `restore`,
  `permission`, `queue_policy`, `model`, `key`, `url`, `startup_args`, `env`,
  `api`, `provider_profile`, `branch_template`, `labels`, `description`,
  `role`, and `watch_paths`.

Observed agent defaults in topology windows:

- `target = "."`.
- `workspace_mode = "git-worktree"` only when the layout leaf uses worktree
  mode; otherwise `inplace`.
- `restore = "auto"`.
- `permission = "manual"`.

Observed maintenance config:

- `maintenance.heartbeat` supports `enabled`, `assessor`, `interval_s`,
  `min_interval_s`, `unknown_streak_cap`, `escalation_policy`, and
  `startup_ensure`.
- Defaults are disabled heartbeat, assessor `ccb_self`, `interval_s = 3600`,
  `min_interval_s = 300`, `unknown_streak_cap = 3`,
  `escalation_policy = "report_only"`, and `startup_ensure = true`.

Manual implication:

- The user manual config reference should be organized by document shape first,
  then topology mode, then agent spec, provider profile, UI/sidebar, tool
  windows, and maintenance.
- The developer manual config chapter should explain how config parsing
  normalizes role ids, expands topology window leaves into agent specs, and
  enforces legacy-vs-topology mutual exclusion.

## Remaining Inventory

Still needed before a complete user manual draft:

- Help output snapshots for every command group.
- Source inventory for `lib/cli/phase2_runtime/` handlers and services.
- Source inventory for default config rendering and rolepack config mutation.
- Tests covering CLI parsing, config validation, role loading, and reload.
- Sanitized runnable examples from an external test project, not from
  `ccb_source`.
