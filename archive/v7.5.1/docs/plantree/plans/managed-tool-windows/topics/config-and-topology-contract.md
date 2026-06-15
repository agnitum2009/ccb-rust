# Config And Topology Contract

Date: 2026-05-30

## Goal

Allow a user to declare a managed Neovim window like this without creating a
fake agent:

```toml
version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"

[tool_windows.neovim]
command = "nvim"
label = "neovim"
show_in_sidebar = true
```

The sidebar should show a `neovim` window row and no `neovim [provider]` child
row. The tool window should be part of CCB's project tmux namespace and reload
planning, but not part of the configured agent set.

## Proposed Model

Add a config model such as:

```python
ToolWindowSpec(
    name: str,
    order: int,
    command: str,
    label: str | None = None,
    show_in_sidebar: bool = True,
)
```

`ProjectConfig` should then carry:

```python
tool_windows: tuple[ToolWindowSpec, ...] = ()
```

The existing `WindowSpec` remains the agent-window model. Keeping a separate
type avoids weakening the current invariant that `[windows]` defines the
configured agent set.

## Config Shape

Preferred user-facing TOML:

```toml
[tool_windows.neovim]
command = "nvim"
label = "neovim"
show_in_sidebar = true
```

Rules:

- `tool_windows` is valid only with `version = 2`.
- Tool window names use the same window-name grammar as `[windows]`.
- A tool window name must not duplicate an agent window name.
- `command` is required and must be a non-empty string.
- `label` defaults to the tool window name.
- `show_in_sidebar` defaults to `true`.
- Tool windows do not declare `provider`, `workspace_mode`, `restore`,
  `permission`, `model`, `key`, `url`, or provider profile fields.
- A `[tool_windows.<name>]` table must not create or modify an
  `[agents.<name>]` table.

## Identity

The topology signature should include the runtime-affecting part of tool
windows because they affect managed tmux topology:

```json
{
  "tool_windows": [
    {
      "name": "neovim",
      "order": 0,
      "command": "nvim"
    }
  ]
}
```

`label` and `show_in_sidebar` are presentation-only. They may hot reload
through project view without recreating the tool pane or changing the config
signature. Provider/runtime identity must not include tool windows. They should
not affect `AgentRegistry`, dispatcher queues, completion tracker state,
provider profiles, or runtime authority records.

## Entry Window

The first slice supports `entry_window` pointing at either an agent window or a
tool window. Tool-window focus selects the tmux window without trying to select
an agent pane.

## Compatibility

Existing compact, hybrid, and `[windows]` configs must continue loading
unchanged. `[windows]` remains the authority for agents. `tool_windows` extends
managed project topology without changing the meaning of any existing
`[windows]` value.
