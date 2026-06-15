# CCBD Agent Hot Reload Open Questions

Date: 2026-05-29

## Questions

- For Phase 6b append-only add-agent, should the narrow pane patcher use a fixed
  default split policy after the anchor pane, or derive a limited append split
  from the new layout spec without touching existing panes?
- How should Phase 6b eliminate the keeper config-signature race during the
  final handoff: a reload-in-progress keeper grace, graph-first publish with
  rollback, or a proven adjacent lease/lifecycle/graph commit helper?
- Should force unload be exposed as `ccb reload --force`, `ccb unload --force`,
  or only through an existing project restart/kill command?
- Should a removed-but-retired agent remain visible in `project_view` for a
  short audit window, or disappear immediately after successful retirement?
