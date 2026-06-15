# Active Anomaly And Hook Attribution

Date: 2026-06-11

## Incident

Manual v7.4.1 testing in `/home/bfly/yunwei/test_ccb2` exposed a false healthy
state:

- `clauder` was shown as idle/completed by CCB.
- The Claude pane still showed scheduled-task and shell-running state.
- The hook event completed `job_86550847f237` after the user had interrupted
  the original CCB turn.
- `ccb_self` could not diagnose the failure because the control-plane evidence
  was already false: the job looked terminal and the agent looked idle.

Root cause:

- Claude `Stop` hook attribution used the latest visible `CCB_REQ_ID` or
  `last-prompt` style evidence from the transcript.
- A later provider-side scheduled-task turn could therefore reuse an earlier CCB
  request id and write a false completion artifact.

Targeted fix:

- Claude hook completion now binds the current assistant stop to the actual
  transcript turn, walks the `parentUuid` chain to the real user prompt, skips
  tool-result user records, and emits a completion artifact only when that
  prompt itself has an outer `CCB_REQ_ID`.
- Scheduled-task, user-interruption, auth/info, and other provider-side turns
  do not inherit an old CCB request id.
- Claude and Gemini hook completion now treat `completed` hook events with an
  empty assistant reply as terminal `incomplete` diagnostics instead of normal
  completion. The diagnostic evidence includes `empty_reply`,
  `empty_provider_reply`, and a short message pointing to transcript, pane, and
  authentication/API checks.
- Codex protocol-turn completion now treats `task_complete` with no boundary
  reply and no prior assistant-visible reply evidence as terminal
  `incomplete`, not normal completion. The diagnostic evidence uses the same
  empty-reply shape and points to protocol session log, pane, and
  authentication/API checks.
- Antigravity (`agy`) pane-quiet completion now treats a visible requested
  done marker with no extracted assistant reply as immediate terminal
  `incomplete`, not normal completion or a long quiet timeout. Diagnostics use
  the same empty-reply shape and include pane snapshot progress fields.
- Project-view pane activity now treats `Running scheduled task`,
  `shell still running`, and `shells still running` as provider-working
  evidence.

Validation already run:

- `python -m pytest -q test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_claude_hook_results.py test/test_ccbd_project_view.py`
  passed with 67 tests.
- `python -m pytest -q test/test_maintenance_heartbeat.py test/test_provider_activity_artifacts.py test/test_v2_config_loader.py test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_ccbd_project_view.py`
  passed with 184 tests.
- `py_compile` passed for the touched hook, transcript, artifact, and activity
  modules.
- `git diff --check` passed.
- Read-only replay of the real stale Claude transcript returned no current CCB
  request id, which is the expected post-fix behavior.
- Source project-view activity classification on the real Claude pane returned
  `active/provider_pane/provider_working`.
- Empty hook completion guard validated with
  `python -m pytest -q test/test_provider_finish_hook_script.py test/test_provider_hook_transcript.py test/test_claude_hook_results.py test/test_gemini_execution_hook.py test/test_claude_execution_polling.py test/test_v2_execution_service.py`
  (`90 passed`).
- Codex/protocol empty-reply guard validated with
  `python -m pytest -q test/test_v2_completion_detectors.py test/test_v2_completion_tracker.py test/test_v2_completion_orchestration.py test/test_codex_execution_polling.py test/test_v2_execution_service.py`
  (`82 passed`).
- Agy pane-quiet empty-reply guard validated with
  `python -m pytest -q test/test_agy_execution_polling.py test/test_v2_completion_detectors.py test/test_v2_completion_tracker.py test/test_v2_completion_orchestration.py test/test_codex_execution_polling.py test/test_v2_execution_service.py test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_claude_hook_results.py test/test_gemini_execution_hook.py`
  (`106 passed`).
- Isolated source-runtime validation on 2026-06-12 used
  `/home/bfly/yunwei/ccb_source/ccb_test` from
  `/home/bfly/yunwei/test_ccb2/ccb_empty_reply_real` with
  `normal:fake`, `codexlike:fake-codex`, `claudelike:fake-claude`,
  `geminilike:fake-gemini`, and `legacy:fake-legacy`. `config validate` and
  startup passed. Empty-reply equivalent terminal reasons produced
  `status: incomplete`: `task_complete_empty_reply`
  (`job_86b92470a01d`), `hook_stop_empty_reply`
  (`job_b7d3a7f0f952`), `hook_after_agent_incomplete`
  (`job_9adaab5ac732`), and `pane_done_empty_reply`
  (`job_c9160d7fcd9d`).
- The same source-runtime validation confirmed route semantics that must not
  be classified as provider empty replies: `--silence` success completed
  normally (`job_1e6a98da6f95`), `--silence` abnormal completion remained
  diagnosable (`job_ff28a17e2680`), standalone `--callback` failed with
  `ask --callback requires an active parent job for the sender`, active-parent
  callback intentionally produced an empty parent reply with
  `completion_reason: callback_pending` (`job_1ee1c42deef4`) and then a normal
  callback continuation (`job_6105fb0a5d2f`), and active-parent
  fire-and-forget `--silence` completed normally (`job_fe59c2e47289`).
- Runtime validation boundary: the fake provider adapter materializes default
  reply text even for abnormal terminal reasons, so true empty-string provider
  reply behavior remains validated by the provider-specific pytest suite:
  `python -m pytest -q test/test_agy_execution_polling.py test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_claude_hook_results.py test/test_gemini_execution_hook.py test/test_v2_completion_detectors.py test/test_v2_completion_tracker.py test/test_v2_execution_service.py test/test_v2_ask_service.py test/test_v2_job_store.py test/test_v2_cli_parser.py`
  (`179 passed`).

## Provider Completion Boundary

Claude hook path:

- Claude completion can be terminalized by hook artifact events.
- The hook must bind the event to the current transcript turn, not to the latest
  historical visible request marker.
- No current-turn request id means no completion artifact.

Codex path:

- Codex pane-backed completion is declared as
  `CompletionSourceKind.PROTOCOL_EVENT_STREAM`.
- Completion authority is the managed Codex protocol/session event log under
  the agent's isolated managed home.
- Codex may have activity/profile hooks, but those hooks are not the terminal
  completion authority for CCB jobs.
- The equivalent Codex risks are not stale `Stop` hook reuse. They are:
  delivery accepted incorrectly, active `CCB_REQ_ID` anchor never observed,
  stale bound session log, unsafe session rebound, pane unable to accept input,
  or pane/control-plane/protocol-log evidence disagreement.

Codex supervision should therefore inspect:

- `delivery_state`, `delivery_started_at`, `delivery_confirmed_at`,
  `delivery_failure_kind`, and `delivery_timeout_s`;
- the active request anchor and whether it appears in the currently bound log;
- whether same-workspace fallback session selection is unique and still under
  the managed Codex session root;
- terminal protocol events, assistant messages, and bound turn/task ids;
- pane evidence such as dead pane, shutdown marker, input box, or unexpected
  provider-working state.

## Default Active-Anomaly Escalation

The heartbeat should wake the configured assessor for read-only diagnosis when
independent evidence disagrees. This must not be implemented as
`mode = "aggressive"` or any parallel behavior branch. The active-anomaly checks
belong to the single default classification path, and normal cadence,
deduplication, escalation-target, and policy gates control noise and authority.

These conditions should be treated as `concern` or `unknown`, not silently
healthy:

- CCB says an agent is idle or a job is terminal, but pane text indicates
  provider work is still running.
- A hook provider rejects current-turn attribution or has terminal hook
  artifacts that conflict with pane state.
- Codex has `pending_anchor` delivery beyond the normal observation window, or
  a delivery failure exposes retryable ambiguity.
- Codex session binding changed while an active submission exists and the
  active request anchor is missing from the expected log.
- An agent has active provider work but no active job, no pending callback, no
  queue, and no clear scheduled activation record.
- A job is active, but no provider-specific progress evidence updates within
  the provider's expected heartbeat or completion window.

V1 escalation remains non-mutating:

- Use `ask --silence` to activate the configured assessor.
- Deduplicate by diagnostic fingerprint.
- Suppress activation when the assessor already has an active maintenance job.
- Enforce minimum interval and repeated-unknown caps.
- Do not auto-run `clear`, `restart`, `cancel`, `repair`, `kill`, or raw tmux
  mutation from heartbeat classification.

## Assessor Evidence Ladder

For active anomalies, the `ccb_self` running-supervision skill should start with
read-only evidence:

1. CCB control-plane snapshot: `ps`, trace, job state, queue state, callback
   state, heartbeat status, and recent maintenance activation records.
2. Provider-specific state: Claude hook event/transcript attribution or Codex
   delivery/session/protocol state.
3. Pane text: metadata, bottom/current `tmux capture-pane`, and bounded
   scrollback around the visible prompt/status area.
4. Short activity sampling: repeated pane captures or process/pane metadata to
   distinguish still-running from stale text.
5. Visual fallback only when text is insufficient, using bounded screenshot
   artifacts as evidence rather than a default polling path.

The purpose is to let `ccb_self` explain and recommend the next step quickly
when CCB's own health summary is overconfident.
