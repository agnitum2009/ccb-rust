# Tool Windows Are Not Agents

Date: 2026-05-30

## Context

The first requested tool-window use case is a default Neovim window shown in
the sidebar as one row. The old agent topology requires every `[windows]` leaf
to declare a provider, which would force `neovim` to become a fake agent if the
feature were implemented only by relaxing `[windows]`.

Fake agents would leak into ask routing, provider runtime, health checks,
completion tracking, Comms, and dynamic reload semantics.

## Decision

Managed tool windows are a separate topology primitive. They are CCB-managed
tmux windows and panes, but they are not agents.

The preferred config shape is `[tool_windows.<name>]` with a command and
optional display label/sidebar visibility. `[windows]` remains the agent-window
topology and continues to define the configured agent set.

## Consequences

- Sidebar can display a tool window as a window row with no child agent row.
- Tool windows do not participate in `ccb ask`.
- Tool windows do not create provider runtime authority, agent registry rows,
  dispatcher queues, completion tracker entries, or provider activity status.
- Namespace and reload code need explicit tool-window paths for tmux creation,
  removal, and CCB identity evidence.
- Future tools such as logs, shells, dev servers, or dashboards can use the
  same model without weakening agent invariants.
