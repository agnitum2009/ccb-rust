# CCB Config Reference

## Authority Files

Effective config precedence is:

1. built-in default config from code;
2. user config at `~/.ccb/ccb.config`;
3. project config at `.ccb/ccb.config`.

Higher layers replace the whole lower-layer config; CCB does not merge partial config documents across layers.

`.ccb/ccb.config` is the highest-priority project config authority. When it is missing, CCB uses `~/.ccb/ccb.config` if present, then the built-in default. CCB does not write a new project config automatically.

Only write `~/.ccb/ccb.config` when the user explicitly wants a user-level or system-wide default CCB team. For ordinary project setup, write `.ccb/ccb.config`.

Do not write `.ccb_config/ccb.config`. That path is legacy residue in older or migrated workspaces. You may read it as migration evidence, but the current config must be created or updated at `.ccb/ccb.config`.

`.ccb/ccb_memory.md` and `.ccb/agents/<agent>/memory.md` are user-editable memory files. They are context, not layout authority.

Do not edit generated runtime state, provider-state homes, `.ccb/provider-profiles/`, `.ccb/ccbd/`, legacy `.ccb_config/`, or generated runtime memory.

## Compact Format

Use compact format for ordinary team layouts:

```text
cmd; main:codex, worker1:codex(worktree); reviewer:claude
```

Leaf tokens:

- `cmd`
- `agent:provider`
- `agent:provider(worktree)`

`cmd` is not an agent. It is the shell pane layout keyword and cannot declare a provider.

Layout operators:

- `;` splits horizontally, left to right. Think columns.
- `,` stacks vertically, top to bottom. Think rows inside a column.
- Parentheses group layout expressions.

Examples:

```text
cmd; main:codex
cmd; main:codex, reviewer:claude
cmd, main:codex; worker1:codex(worktree), reviewer:claude
cmd; main:codex, worker1:codex(worktree), worker2:claude(worktree); reviewer:claude, discuss:codex
```

Compact config requires providers on agent leaves. Bare `main` is not valid in compact config; write `main:codex`.

## Workspaces

Compact workspace syntax:

```text
worker1:codex(worktree)
```

Meaning:

- no suffix: `workspace_mode = "inplace"`;
- `(worktree)`: `workspace_mode = "git-worktree"`.

`git-worktree` requires the project root to be a git repository. If the project is not a git repository, ask before using isolation. Do not silently replace it with copy mode.

`workspace_mode = "copy"` is available only in rich TOML and should be used only when explicitly requested.

Worktree branch naming can be customized with `branch_template`, but do not set it by default. Supported variables are `{agent_name}`, `{project_slug}`, and `{date}`. The default branch template is `ccb/{agent_name}`.

## Hybrid Format

Use hybrid format when the compact layout is enough but one or more agents need extra fields:

```toml
cmd; main:codex, worker1:codex(worktree); reviewer:claude

[agents.main]
description = "Coordinates planning, progress, and delegation."
model = "gpt-5"

[agents.reviewer]
description = "Reviews behavior, tests, risks, and regressions."
```

The compact header owns:

- layout;
- `default_agents`;
- `cmd_enabled`;
- agent `provider`;
- agent `workspace_mode`.

Hybrid overlay rules:

- only `[agents.<name>]` tables are allowed;
- each overlay agent must already exist in the compact header;
- overlay must not redefine `provider` or `workspace_mode`;
- use overlay for fields such as `model`, `key`, `url`, `description`, `labels`, `startup_args`, `provider_profile`, `permission`, `restore`, `queue_policy`, `branch_template`, and `watch_paths`.

## Rich TOML

Use rich TOML when compact/hybrid cannot express the request, especially named managed windows, sidebar settings, or explicit `workspace_mode = "copy"`.

Named windows topology:

```toml
version = 2
entry_window = "main"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20

[windows]
main = "main:codex, reviewer:claude"
build = "worker1:codex(worktree), worker2:codex(worktree)"
```

Rules for `windows` topology:

- `windows` keys are managed tmux window names.
- window values use the same layout operators and `agent:provider` leaves as compact config.
- `cmd` is not supported inside `windows`; manual shell panes are outside CCB config and are not shown in the sidebar.
- Do not set `layout`, `cmd_enabled`, or `default_agents` together with `windows`.
- every configured agent must appear in exactly one window.
- `entry_window` is optional and defaults to the first configured window.
- `[ui.sidebar]` is optional and defaults to `mode = "every_window"`, `width = "15%"`, `bottom_height = 20`.
- set `mode = "off"` to disable projected sidebar panes.

Classic rich TOML with top-level `layout` is still available for fields that compact/hybrid cannot express, for example explicit `workspace_mode = "copy"`:

```toml
version = 2
default_agents = ["main", "worker1"]
cmd_enabled = true
layout = "cmd; main, worker1"

[agents.main]
provider = "codex"
target = "."
workspace_mode = "inplace"
restore = "auto"
permission = "manual"

[agents.worker1]
provider = "codex"
target = "."
workspace_mode = "copy"
restore = "auto"
permission = "manual"
```

## Provider And Model Fields

Default behavior should inherit provider credentials/config from the user's normal provider home.

Use `key` and `url` only when the user explicitly wants an agent-local API route. These shortcuts are supported for `codex`, `claude`, and `gemini`.

Use `model` only when the user wants a provider model override. Model shortcuts are supported for `codex`, `claude`, `gemini`, and `opencode`.

Do not mix `key` or `url` with provider API env fields under `agents.<name>.env` or `agents.<name>.provider_profile.env`.

Use `provider_profile` only for advanced inheritance or environment behavior. Do not create `.ccb/provider-profiles/` directories manually.

## Agent Names

Agent names must match:

```text
^[a-zA-Z][a-zA-Z0-9_-]{0,31}$
```

Names are normalized to lowercase.

Reserved names include:

```text
all, from, user, system, ask, cancel, pend, ping, watch, kill, ps, logs, doctor, config, cmd, version, update, help
```

Prefer role names over generic names:

- `main`
- `worker1`, `worker2`
- `reviewer`
- `discuss`
- `research`
- `qa`
- `docs`

## Common Topologies

Light engineering team:

```text
cmd; main:codex, worker1:codex(worktree); reviewer:claude
```

Full parallel team:

```text
cmd; main:codex, worker1:codex(worktree), worker2:codex(worktree), worker3:claude(worktree); reviewer:claude, discuss:codex
```

Multi-provider research and implementation:

```text
cmd; main:codex, builder:codex(worktree), research:gemini; reviewer:claude
```

Two Codex agents with different explicit API routes:

```toml
cmd; fast:codex, deep:codex

[agents.fast]
key = "sk-fast..."
model = "gpt-5-mini"

[agents.deep]
key = "sk-deep..."
url = "https://api.example.com/v1"
model = "gpt-5"
```

Never include real secrets in public repositories.
