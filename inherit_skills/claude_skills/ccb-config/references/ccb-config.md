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

Do not edit memory files for ordinary config design, migration, provider changes, worktree changes, or window layout changes. Update memory only when the user explicitly asks for workflow memory design or role guidance changes.

Do not edit generated runtime state, provider-state homes, `.ccb/provider-profiles/`, `.ccb/ccbd/`, legacy `.ccb_config/`, or generated runtime memory.

## Compact Format

Use compact format for ordinary single-window team layouts, especially when the user wants a persistent `cmd` pane. Existing compact configs remain single-window configs and do not need migration to `[windows]`:

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

Use hybrid format when the compact single-window layout is enough but one or more agents need extra fields. Hybrid configs without `[windows]` also remain single-window configs:

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

## Explicit Windows Topology

Use windows topology when the user wants named tmux windows, per-window agent grouping, or native sidebar layout across multiple windows:

```toml
version = 2
entry_window = "main"

[windows]
main = "main:codex"
work = "worker1:codex(worktree), worker2:codex(worktree), worker3:claude(worktree)"
review = "reviewer:claude, discuss:codex"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20

[ui.sidebar.view]
agents_height = "33%"
comms_limit = 5
comms_compact = true
tips_enabled = true
tips = [
  "C-b d  detach",
  "C-b h/j/k/l pane",
  "C-b H/J/K/L resize",
  "C-b o  next pane",
  "C-b z  zoom",
  "C-b w  tree",
  "C-b n/p next/prev",
  "C-b 0-9 jump win",
  "C-b [  copy mode",
  "copy: PgUp/PgDn",
  "copy: v select",
  "copy: y yank",
  "copy: q exit",
  "C-b ]  paste",
  "C-b c  new win",
  "C-b ,  rename",
  "C-b ?  keys",
]
```

Rules:

- Only `[windows]` enables multi-window topology. Do not rewrite an existing compact/hybrid config into `[windows]` unless the user asks for named windows or per-window grouping.
- `[windows]` owns layout and the effective configured-agent set.
- Each configured agent must appear in exactly one window layout.
- Window layout leaves must declare providers: `agent:provider` or `agent:provider(worktree)`.
- `cmd` is not supported inside `[windows]` topology. Use compact/hybrid config when a persistent command pane is required.
- Do not combine windows topology with `default_agents`, `layout`, or `cmd_enabled`.
- `entry_window` is optional; it defaults to the first window.
- `[ui.sidebar]` is optional. Defaults are `mode = "every_window"`, `width = "15%"`, and `bottom_height = 20`.
- Agent leaves provide default provider and workspace mode. Same-name `[agents.<name>]` tables are overlays; they may override fields such as `workspace_mode`, and the provider there must match the provider in `[windows]` if it is repeated.
- `[agents.<name>]` tables for names no longer present in `[windows]` are ignored as stale overlay residue.
- `[ui.sidebar.view]` is optional and UI-only. It can tune sidebar tree height, Comms visible row count/compactness, and short Tips text without changing the managed window topology.

## Migrating Old Configs To Windows

Old compact and hybrid configs are still valid single-window configs. Migrate them only when the user asks for multi-window behavior, named windows, or per-window sidebar layout.

Migration rules:

- ask one concise target-shape question when needed: number of windows and rough grouping;
- preserve agent names, providers, worktree markers, and ordering unless the user asks to redesign roles;
- preserve TOML overlay fields by moving them under the same `[agents.<name>]` table after `[windows]`;
- preserve memory files without editing them unless the user explicitly asks for workflow memory changes;
- remove `cmd` from the migrated layout because `[windows]` does not support the persistent command pane;
- choose concise workflow window names such as `main`, `work`, `review`, `research`, or `ops`;
- keep each agent in exactly one window;
- keep compact/hybrid format if the requested change is only a single-window pane rearrangement.

Compact to windows example:

```text
cmd; main:codex, worker1:codex(worktree), worker2:claude(worktree); reviewer:claude
```

becomes:

```toml
version = 2
entry_window = "main"

[windows]
main = "main:codex"
work = "worker1:codex(worktree), worker2:claude(worktree)"
review = "reviewer:claude"
```

Example with an override:

```toml
version = 2

[windows]
main = "main:codex"
work = "worker1:codex(worktree)"

[agents.worker1]
model = "gpt-5"
description = "Implements coherent code changes in an isolated git worktree."
```

## Legacy Rich TOML

Use legacy rich TOML only when compact/hybrid or windows topology cannot express the request, for example explicit `workspace_mode = "copy"`:

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

## Skill Inheritance

CCB config supports `agents.<name>.provider_profile.inherit_skills`, which enables or disables inheritance of the provider source-home skills as a whole. It does not support a per-skill allowlist in `.ccb/ccb.config`.

Durable skill installs belong in the provider source home:

- Codex: `${CODEX_HOME:-$HOME/.codex}/skills/<skill>`
- Claude: `$HOME/.claude/skills/<skill>`

After restart or relaunch, managed agents for that provider inherit those skills when `inherit_skills` is true. This may affect all same-provider agents, not only one agent.

If the user asks for one-agent-only skill injection, explain that it is not a durable config-level feature yet. A temporary copy into `.ccb/agents/<agent>/provider-state/.../skills` may work for an already-mounted agent, but it is runtime/provider-state and can be replaced by projection refresh or restart. Use it only after explicit confirmation, and never write through a symlink.

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

Full parallel team with named windows and native sidebars:

```toml
version = 2
entry_window = "main"

[windows]
main = "main:codex"
work = "worker1:codex(worktree), worker2:codex(worktree), worker3:claude(worktree)"
review = "reviewer:claude, discuss:codex"
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
