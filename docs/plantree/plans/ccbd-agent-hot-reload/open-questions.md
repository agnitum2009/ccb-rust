# CCBD Agent Hot Reload Open Questions

Date: 2026-05-29

## Questions

- How should a new agent pane be inserted into an existing window layout when
  the target layout is more specific than "append to window" and the existing
  panes must not move?
- Should successful reload update `start-policy.json` with the latest
  auto-permission policy, or should start policy remain owned only by `ccb`
  startup/restore commands?
- What are the first default values for drain timeout, pending replacement
  queue length, and retained old service graph count?
- Should force unload be exposed as `ccb reload --force`, `ccb unload --force`,
  or only through an existing project restart/kill command?
- Should a removed-but-retired agent remain visible in `project_view` for a
  short audit window, or disappear immediately after successful retirement?
