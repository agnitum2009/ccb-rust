# Sidebar Tips Layout Plan

Date: 2026-05-27

## Purpose

Plan the next CCB native sidebar slice: compress the existing Comms panel,
reserve the top panel for the window/agent tree, and add a bottom Tips panel
for short tmux operating hints.

This plan is scoped to sidebar presentation and UI-only configuration. It does
not change agent lifecycle authority, message/job authority, or namespace pane
ownership.

## File Map

- [roadmap.md](roadmap.md): current implementation sequence and phase gates.
- [open-questions.md](open-questions.md): unresolved product and config
  questions only.
- [topics/three-panel-sidebar.md](topics/three-panel-sidebar.md): layout,
  rendering, config, hot-reload, and testing notes.
- [decisions/001-sidebar-view-config-is-ui-only.md](decisions/001-sidebar-view-config-is-ui-only.md):
  decision record for keeping tips/layout display settings out of topology
  authority.

## Related Sources

- [../../../ccb-agent-sidebar-integration-plan.md](../../../ccb-agent-sidebar-integration-plan.md)
- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../baseline/runtime-flows.md](../../baseline/runtime-flows.md)

## Scope

In scope:

- Three-panel sidebar layout: window/agent tree, compact Comms, Tips.
- Comms visible item limit, initially 5.
- Compact one-line Comms rows by default.
- Default Tips content with short tmux key hints.
- Optional `.ccb/ccb.config` UI-only overrides with hot reload through
  `project_view`.

Out of scope:

- Provider execution-state authority changes.
- New ask/restart/reflow controls.
- Cross-project dashboard.
- Changing tmux pane geometry beyond existing sidebar width.
- True terminal font-size control.
