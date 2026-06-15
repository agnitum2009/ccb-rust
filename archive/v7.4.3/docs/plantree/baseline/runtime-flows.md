# Runtime Flows

Date: 2026-05-25

## Startup And Attach

Observed contract shape:

1. The user runs `ccb` from a project.
2. CCB resolves config from built-in default, `~/.ccb/ccb.config`, then
   `.ccb/ccb.config`.
3. `ccbd` owns the project backend and materializes the project tmux namespace.
4. Configured agents are mounted into the project namespace.
5. The foreground command attaches to the project workspace.

The README should explain this as a user workflow, not as daemon internals.

## v7 Window And Sidebar Flow

Observed v7 contract shape:

1. A rich config can declare `version = 2`.
2. `[windows]` defines named managed tmux windows.
3. `entry_window` selects the initial window.
4. `[ui.sidebar]` can project the native sidebar into managed windows.
5. The sidebar presents project windows, agents, activity, and Comms state while
   focus changes go through CCB authority.

## Ask Flow

Observed README behavior:

1. Users can ask another named agent explicitly with `/ask <agent> ...`.
2. Agents can use the `ask` skill or CLI routes for CCB-native delegation.
3. During an active CCB ask task, callback chaining uses `ccb ask --callback`
   when the child result is required.
4. Fire-and-forget work should submit once and stop.

## Shutdown And Rebuild

Observed public command set:

- `ccb kill` stops the current project backend.
- `ccb kill -f` force-cleans project residue before rebuild.
- `ccb -n` rebuilds runtime state while preserving config and same-name managed
  agent history.
- Exact troubleshooting command wording should be verified against current CLI
  help before publishing new README examples.

