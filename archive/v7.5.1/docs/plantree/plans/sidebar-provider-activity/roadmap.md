# Sidebar Provider Activity Roadmap

Date: 2026-05-27

## Done

- Diagnosed that current sidebar activity mostly uses CCB job state, runtime
  health, pane liveness, and pane text heuristics.
- Confirmed Claude CCB-managed jobs can already terminalize some API errors
  through session event log handling, but manual pane activity is not wired into
  sidebar status.
- Inspected `tmux-agent-status` and recorded the useful hook-status pattern:
  provider hooks write small scoped status files and UI reads aggregated state.
- Decided not to import `tmux-agent-status` as a tmux plugin or global scanner.
- Drafted the Codex activity topic and shared validation matrix under this
  plan-tree root.
- Documented current `ccbd` Comms status, message-bureau lineage, automatic
  retry, manual retry, and recovery behavior in
  [topics/current-ccbd-comms-and-retry.md](topics/current-ccbd-comms-and-retry.md).
- Added a dedicated mailbox-internal design reference index in
  [topics/mailbox-internal-design-references.md](topics/mailbox-internal-design-references.md)
  so future status work does not confuse sidebar Comms rows with mailbox-kernel
  policy.
- Recorded that provider-native activity is the execution-state authority, while
  CCB job/Comms state remains workflow metadata.
- Recorded that provider `failed` should remain sticky until the next provider
  turn or runtime ownership change.

## In Progress

- Define and implement the phase-1 provider status bridge in
  [topics/phase-1-provider-status-to-sidebar.md](topics/phase-1-provider-status-to-sidebar.md):
  provider hook/session facts -> agent runtime activity artifact ->
  `project_view` agent row -> sidebar symbol.

## Next

- Add shared activity artifact reader/writer contract with project, agent,
  provider, runtime session, pane, workspace, and freshness validation.
- Add `project_view` resolver tests for fresh, stale, sticky-failed,
  wrong-provider, wrong-agent, wrong-session, and wrong-pane activity artifacts.
- Implement provider-owned hook helpers for Codex and Claude activity events.
- Materialize Codex managed-home activity hooks after a hook payload/trust probe.
- Materialize Claude activity hooks for `UserPromptSubmit`, tool events, `Stop`,
  `Notification`, background tasks, and API errors.
- Run the validation matrix in [topics/test-matrix.md](topics/test-matrix.md),
  including live `/home/bfly/yunwei/test_ccb2` stable and fault lanes.

## Deferred

- Codex app-server as a future transport backend.
- Process-tree polling fallback except as bounded diagnostics.
- Sidebar detail popups for provider reason text.
- Cross-project provider activity aggregation.

## Release Gate

This work is release-ready only when:

- manual Codex and Claude work no longer appears idle while active;
- CCB-managed job state still wins over provider activity evidence;
- API/auth/model failures surface as failed or recoverable pending states;
- no stale activity can keep an agent active forever;
- `project_view` refresh CPU cost remains bounded;
- live fault-lane tests have been recorded for disconnect, invalid auth, and
  unavailable model.
