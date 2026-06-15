# Sidebar Config Entry

Date: 2026-06-06

## Goal

Expose the config UI from the native sidebar after `ccb config ui` exists and
works independently.

## Desired UI

The top-right sidebar tree controls should remain icon-only:

```text
⚙ ↻ ×
```

- `⚙`: open config UI.
- `↻`: restart configured agent panes.
- `×`: project-level kill.

Do not add visible `r`, `q`, or text buttons to the sidebar chrome.

## Launch Behavior

First implementation can spawn the sibling `ccb` binary:

```bash
ccb config ui --project <project_root>
```

The sidebar helper should not block the TUI while the UI command is running.
It should show a concise status or error if launch fails.

Later implementation may route through a daemon RPC if launch status,
single-instance behavior, or richer diagnostics are needed.

## Fallback Behavior

If automatic browser opening fails, the config UI command should print or return
the local URL. The sidebar should surface enough text for the user to copy it,
for example:

```text
config ui: http://127.0.0.1:49231/?token=...
```

## Safety

- The sidebar button launches only the same local config editor command.
- It must not write config directly.
- It must not run reload or restart the project.
- Existing `↻` and `×` behavior must remain unchanged.

## Test Targets

- Header control hit testing with three controls.
- Config icon spawns the expected command without blocking.
- Spawn failure displays a sidebar error.
- Existing restart and kill clicks still hit their original actions.
- No keyboard shortcut labels appear in the sidebar header.
