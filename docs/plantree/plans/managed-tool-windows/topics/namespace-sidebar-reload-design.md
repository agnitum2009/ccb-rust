# Namespace, Sidebar, And Reload Design

Date: 2026-05-30

## Namespace Materialization

Tool windows should be CCB-managed tmux windows with project/session-scoped
identity. Cold start should:

1. create the tmux window using the tool window name;
2. create the sidebar pane when `[ui.sidebar].mode = "every_window"`;
3. mark the tool pane with CCB identity options:
   - `@ccb_project_id`
   - `@ccb_managed_by=ccbd`
   - `@ccb_window=<tool-name>`
   - `@ccb_role=tool`
   - `@ccb_slot=tool:<tool-name>`
   - namespace epoch and socket evidence matching other managed panes;
4. start the command in the tool pane, preferably through the same bounded tmux
   send/respawn primitives used by existing namespace code;
5. leave agent pane materialization untouched.

The materializer should not call provider start flow, mount manager, runtime
supervisor, completion tracker, dispatcher, or agent restore logic for tool
windows.

## Project View Shape

Project view should emit tool windows as windows, not agents:

```json
{
  "name": "neovim",
  "label": "neovim",
  "kind": "tool",
  "show_in_sidebar": true,
  "agents": [],
  "focus": false
}
```

Existing agent windows can either omit `kind` for compatibility or emit
`kind = "agents"`. The sidebar can keep the existing rendering shape: render
the window row, then render zero matching child agent rows.
`show_in_sidebar = false` hides the row only; it does not suppress the
project-owned sidebar pane for that tmux window when sidebar mode is
`every_window`.

Click/focus behavior should focus the tmux window for a tool row. It should not
try to select an agent pane.

## Sidebar Rendering

The first UI target is intentionally simple:

- show exactly one row for `neovim`;
- do not render a child row;
- do not show provider status, queue state, or Comms affordances for the tool;
- allow focus/click/keyboard selection to switch to the tool window.

This avoids inventing fake status semantics. Tool liveness or command-exited
markers can be added later as a tool-specific enhancement.

## Reload Planning

Extend reload classification with tool-specific operations:

| Operation | First behavior |
| :--- | :--- |
| `add_tool_window` | Create new managed tmux window/sidebar/tool pane, then publish config graph. |
| `remove_tool_window` | Kill only the matching managed tool window/pane when it is CCB-owned and not an agent window. |
| `change_tool_window` | Block in first slice; require explicit future restart policy. |

`add_tool_window` and `remove_tool_window` should be independent of agent
runtime authority. They can reuse the existing reload transaction for config
signature, lifecycle, graph publish, cache invalidation, and keeper handoff.

## Safety Invariants

- Adding or removing a tool window must not kill, split, resize, or send input
  to existing agent panes.
- Tool windows must never become configured agents.
- Tool windows must not appear in `ccb ask` targets.
- Removing a tool window must require managed CCB identity proof before tmux
  mutation.
- Failed reload must leave the old graph/config visible.
- CCB-owned tmux behavior remains project/session-scoped and must not depend on
  external tmux or terminal configuration.

## First Implementation Slices

1. Config/load/project-view/sidebar payload only.
2. Cold-start namespace materialization for tool windows.
3. Explicit reload dry-run classification and render output.
4. Explicit reload add/remove mutation.
5. Manual `test_ccb2` validation with `neovim`.
