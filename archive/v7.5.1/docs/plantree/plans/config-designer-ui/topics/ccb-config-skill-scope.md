# CCB Config Skill Scope

Date: 2026-06-06

## Goal

Make the `ccb-config` skill a precise `.ccb/ccb.config` assistant. It should
explain and edit supported configuration fields, but it should not become a
workflow-memory designer by default.

## Required Behavior

The skill should:

- resolve the active config authority first;
- read the current `.ccb/ccb.config` when present;
- show a concise menu/list of supported configuration areas;
- propose one complete TOML config or a clear patch to the existing config;
- validate with the current config loader after writing;
- remind the user to use `ccb reload --dry-run`, `ccb reload`, or a restart as
  appropriate.

The skill should not:

- edit `.ccb/ccb_memory.md` or `.ccb/agents/<agent>/memory.md` during ordinary
  config work;
- invent workflow roles or memory blocks unless explicitly requested;
- write runtime state, provider-state homes, installed role stores, or generated
  memory files;
- run `ccb`, `ccb -s`, `ccb kill`, or restart commands from inside the skill.

## Menu Grouping

The skill should present supported knobs in a structured way:

### Basic

- config source and target file;
- windows topology and `entry_window`;
- window names and agent placement;
- agent names and providers;
- Role Pack bindings;
- managed Neovim tool window;
- sidebar mode and basic layout.

### Workspace

- `inplace`;
- `git-worktree`;
- `workspace_group`;
- `workspace_path`;
- `branch_template`.

### Model And API

- `model`;
- `key`;
- `url`;
- legacy nested `[agents.<name>.api]` only as migration input.

### Provider Advanced

- `provider_profile` inheritance flags;
- provider env;
- `provider_command_template`;
- `startup_args`.

### Runtime

- `permission`;
- `restore`;
- `queue_policy`;
- `labels`;
- `description`;
- `watch_paths`.

## Default Proposal Rules

- Prefer `version = 2` `[windows]` topology.
- Treat `[windows]` as the only authority for agent presence, provider,
  default `inplace`/`git-worktree` workspace mode, order, and window grouping.
- Use `[agents.<name>]` only for overlays such as model/API, provider profile,
  role binding for custom local names, runtime policy, descriptions, labels,
  and advanced workspace fields. Do not write redundant `provider`,
  `workspace_mode = "inplace"`, or `workspace_mode = "git-worktree"` fields in
  overlays.
- Include `[tool_windows.neovim]` by default unless the user asks to disable it.
- Disable Neovim by removing the `[tool_windows.neovim]` block, not by writing
  `enabled = false`.
- Keep provider credentials inherited by default.
- Do not write secrets unless the user explicitly provides and accepts that they
  will be stored in config.

## Output Shape

For non-trivial changes, the skill should return:

1. current config source and summary;
2. selected options or changed menu items;
3. full TOML preview;
4. validation command/result after writing;
5. next runtime action reminder.
