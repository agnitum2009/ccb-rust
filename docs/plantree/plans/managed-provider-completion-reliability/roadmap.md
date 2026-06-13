# Managed Provider Completion Reliability Roadmap

Date: 2026-06-12

## Status Summary

- Current status: P0 implementation is in the working tree and folded into the
  pending `7.5.0` release candidate; not committed or pushed.
- Last verified: adjacent Claude/provider completion regression suite
  (`114 passed`), `python -m compileall -q lib bin ccb`, `git diff --check`,
  `ccb_test --diagnose`, isolated `ccb_test --version`, isolated
  `ccb_test config validate`, npm pack dry-run, Markdown local link check, and
  reviewer1 code review PASS (`job_1c92c6c961d1`).
- New Codex prompt-delivery incident is shaped as a planning topic; no
  implementation has landed for that slice yet.
- Next target: finish release-candidate validation and review for the Claude P0
  slice, then implement Codex binding evidence and delivery preflight before
  any automatic retry policy.

## Done

- Captured the Claude pane-backed incident: two Claude-backed jobs produced
  visible assistant replies and `assistant_chunk` events with
  `stop_reason = "end_turn"`, but CCB stayed `terminal=false` until
  `completion_timeout`.
- Received `worker1` code analysis. It identified that Claude
  `stop_reason=end_turn` is parsed and included in `ASSISTANT_CHUNK`, but the
  Claude state machine only emits `TURN_BOUNDARY` for `CCB_DONE` text or
  `system/turn_duration`.
- Received `reviewer1` ask-system review. It confirmed the same P0 bug and
  raised related risks: `SessionBoundaryDetector` empty boundary completion,
  timeout-with-reply semantics, and heartbeat visibility for
  provider-finished-but-not-terminal states.
- Chose the first repair boundary: fix Claude state-machine terminal evidence
  and session-boundary empty reply behavior before broader timeout or heartbeat
  policy changes.
- Implemented the P0 working-tree slice:
  - Claude primary assistant `stop_reason=end_turn` with non-empty reply and an
    observed anchor now emits `TURN_BOUNDARY(reason=assistant_end_turn)`.
  - `SessionBoundaryDetector` now treats an empty `TURN_BOUNDARY` with no prior
    assistant reply as `incomplete/task_complete_empty_reply` with
    `empty_reply` and `error_type=empty_provider_reply` diagnostics.
  - Focused tests cover primary `end_turn`, subagent `end_turn`, `tool_use`,
    empty `end_turn`, existing `CCB_DONE`, `turn_duration`, and empty
    session-boundary handling.
- Completed reviewer1 code review for the P0 slice with PASS and no blocking
  issues. The review confirmed the `end_turn` guards, duplicate-boundary guard,
  empty boundary behavior, and deferral of silence/callback/session-rotation
  hardening to the next validation phase.
- Added public release-note coverage for the Claude `stop_reason=end_turn`
  terminalization and `SessionBoundaryDetector` empty boundary guard under the
  release candidate line now targeting `v7.5.0`.
- Captured the Codex prompt-delivery boundary: worker mailbox events can be
  consumed while the managed Codex session log never records the active
  `CCB_REQ_ID`; the current failure terminalizes as
  `codex_prompt_delivery_failed / delivery_anchor_missing`.
- Added the Codex repair plan:
  [topics/codex-prompt-delivery-binding-drift.md](topics/codex-prompt-delivery-binding-drift.md).

## In Progress

- Broaden P0 validation only where it exercises existing contracts: tracker
  terminalization, reliability timeout suppression after terminal evidence,
  silence delivery suppression, callback routing, and session rotation.

## Next

1. Add tracker/dispatcher-level regression coverage proving accepted Claude
   `end_turn` terminal evidence prevents the 900-second reliability timeout
   path.
2. Add or run existing silence and callback-chain regressions to confirm earlier
   terminalization does not change delivery suppression or callback routing
   semantics.
3. Add session-rotation regression coverage for stale detector state before a
   later anchored Claude `end_turn`.
4. Run any remaining release-gate provider completion suites before
   merge/release handoff.
5. Record any review findings or policy changes before broadening timeout or
   heartbeat behavior.
6. Implement Codex binding evidence for dead/stale `codex.pid`, bridge PID,
   pane PID mismatch, session log freshness, and provider activity freshness.
7. Add a Codex delivery preflight gate so known-stale bindings fail as
   retryable provider-runtime errors before tmux prompt paste.
8. Surface `delivery_anchor_missing` as degraded provider health evidence in
   doctor/project-view/maintenance paths with guarded restart guidance.

## Deferred

- Decide timeout-with-reply semantics. `completion_timeout` with a non-empty
  reply may need richer diagnostics, but should not be reclassified as
  `completed` without a separate decision.
- Add maintenance heartbeat suspicion for provider reply evidence present while
  CCB remains non-terminal.
- Revisit callback and silence interaction only after the P0 repair is stable.
- Decide whether `stop_sequence` or `max_tokens` ever constitute terminal
  evidence for Claude; they are intentionally excluded from the first slice.
- Decide whether Codex `delivery_anchor_missing` can ever trigger automatic
  restart and retry, or whether retry must remain explicit to avoid duplicate
  downstream side effects.
