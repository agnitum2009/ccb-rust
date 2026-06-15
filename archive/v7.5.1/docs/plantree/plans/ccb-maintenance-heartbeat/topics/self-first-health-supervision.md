# Self-First Health Supervision

Date: 2026-06-11

## Goal

Improve fault detection without making `ccbd` overly aggressive or turning the
maintenance classifier into a large provider-specific rule tree.

The target split:

- `ccbd` remains conservative and deterministic. It owns authoritative job
  state, runtime lifecycle, explicit terminal failures, and no-progress fuses.
- CCB maintenance owns cheap read-only suspicion detection and dispatch.
- `ccb_self` owns semantic diagnosis, deeper evidence gathering, explanation,
  bounded repair selection, and follow-up scheduling.

## Design Principle

CCB should not try to fully diagnose every gray-zone condition. It should detect
that evidence is inconsistent, degraded, stale, or ambiguous, then package a
small evidence envelope for `ccb_self`.

This keeps the default path unified:

- no `mode = "aggressive"`;
- no parallel classifier behavior;
- no fine-grained diagnosis matrix inside ccbd;
- no automatic mutating repair from heartbeat classification.

## Three Lanes

### Hard Authority Lane

Handled directly by dispatcher/provider execution:

- explicit provider terminal success or failure;
- pane death during active polling;
- runtime binding failures;
- Codex delivery-anchor failure after strict guards;
- provider no-terminal timeout;
- completion tracker timeout;
- job heartbeat timeout.

These can terminalize jobs because the evidence is deterministic enough for the
control plane.

### Gray-Zone Lane

Handled by maintenance heartbeat as suspicion envelopes:

- control-plane state and pane/provider evidence disagree;
- evidence needed for a confident decision is missing, stale, or contradictory;
- the job is running but progress evidence is too weak to classify;
- the job is terminal but the provider pane still looks active;
- a queue, callback, reply delivery, or scheduled activation is stuck but not
  conclusively failed;
- a provider-specific source is ambiguous, such as hook attribution rejection,
  Codex `pending_anchor`, or session rebound uncertainty.

The gray-zone lane should produce `concern` or `unknown`, activate the
configured assessor, and schedule bounded follow-up.

### Self Repair Lane

Handled by `ccb_self` through sanctioned commands and skills:

- inspect pane text and short activity samples;
- inspect traces, completion snapshots, provider hook/log state, message bureau
  state, and maintenance records;
- classify likely domain: provider, pane, config, queue, callback, reply
  delivery, session binding, or unknown;
- recommend or perform bounded repairs when policy allows.

V1 repairs should be explicit, narrow, and reversible:

- `ccb restart <agent>` for a known bad single agent slot after busy gates pass;
- `ccb repair retry|resubmit|ack` for message/reply lineage recovery;
- `ccb config validate`, `ccb reload --dry-run`, `ccb reload`, and targeted
  agent restart for config/provider fixes;
- `ccb maintenance schedule --after ...` for follow-up;
- no raw tmux mutation except read-only capture/sampling in diagnostics;
- no restart-all, kill, force cleanup, or business-task continuation.

## Suspicion Envelope

The heartbeat payload should be coarse and stable. It should say "this needs
semantic assessment", not "this is definitely the root cause".

Recommended fields:

- `condition_kind`: broad kind such as `state_conflict`, `progress_stale`,
  `delivery_ambiguous`, `provider_evidence_degraded`, `queue_or_callback_stuck`,
  or `manual_diagnose`.
- `target_agent`, optional `job_id`, optional `message_id`, optional
  `reply_delivery_job_id`.
- `control_state`: job status, comms business status, runtime state, activity
  state/reason/source.
- `provider_state`: provider name plus exported safe fields such as Codex
  `delivery_state`, `anchor_seen`, session path, timeout deadline, and
  completion source.
- `pane_ref`: pane id and whether a short capture/sample is attached or
  available on demand.
- `evidence_refs`: trace id/path, completion snapshot path, provider event/log
  refs, heartbeat status, activation id, and recent terminal decision reason.
- `confidence`: `low`, `medium`, or `high` confidence that semantic assessment
  is needed, not confidence in a root cause.
- `allowed_actions`: bounded CCB actions that self may consider for this
  envelope.

Avoid putting large raw logs in the envelope. Use references and let `ccb_self`
pull deeper evidence only when needed.

## Implemented Slice 1

The first code slice keeps the classifier coarse:

- `provider_work_without_control_work`: emitted when project-view shows
  provider/pane work for an agent but there is no current CCB job and no active
  communication targeting that agent. This covers the important false-healthy
  class where CCB has already gone idle/terminal but the pane still looks busy.
- `degraded_activity_evidence`: emitted when an active or pending activity row
  lacks source or reason evidence. This is `unknown`, not a provider-specific
  diagnosis.

The emitted evidence kind is `suspicion_envelope`. It carries:

- `condition_kind`;
- `control_state`;
- `provider_state`;
- `pane_ref`;
- `evidence_refs`;
- `confidence = needs_self_assessment`;
- read-only-first `allowed_actions`.

The activation message now tells `ccb_self` to use only actions explicitly
allowed by the diagnostic package and to avoid restart/repair unless the
package allows it and duplicate business work is ruled out.

Current slice limits:

- Hook-attribution rejection, Codex `pending_anchor`, session rebound ambiguity,
  and no-terminal timeout risk need the provider execution runtime state exposed
  in the next slice.
- The classifier still does not inspect raw tmux, provider logs, or private
  provider state directly; those remain `ccb_self` responsibilities.
- Current `allowed_actions` contains only read-only diagnosis, follow-up
  scheduling, and user escalation. Future mutating actions such as restart or
  repair must be policy-gated structurally, not only by prompt wording.

## Implemented Slice 2

The second code slice exposes safe provider execution runtime state through
`project_view`:

- `ExecutionService.active_runtime_snapshots()` returns one bounded snapshot
  per active provider execution.
- `ProjectViewService` attaches the matching snapshot to each agent row as
  `provider_runtime`.
- When a current CCB job exists, project-view attaches only the provider
  runtime snapshot whose job id matches that current job. When no current job
  exists, a single runtime snapshot is treated as orphan evidence; multiple
  orphan snapshots are surfaced as a conflict summary rather than silently
  last-write-wins.
- The snapshot is white-listed and scalar-only. It includes request anchor,
  source kind, provider, primary completion authority, delivery state,
  anchor-seen flags, session/completion refs, delivery timeout/deadline,
  last-progress timestamp, and no-terminal timeout/deadline.
- It avoids volatile age counters in project-view. The classifier computes
  observation age from project-view `generated_at` plus delivery timestamps.
- It deliberately excludes runtime objects and large/private text such as
  backend, reader, prompt text, reply buffers, assistant buffers, and raw
  provider state.

The default classifier now uses `provider_runtime` for two coarse gray-zone
conditions:

- `provider_runtime_without_control_job`: execution runtime exists but the
  agent row has no current CCB job.
- `provider_delivery_pending_anchor`: provider delivery remains
  `pending_anchor` past a short observation window and the anchor is still not
  seen.

The envelope copies stable provider runtime evidence only. `ccb_self` can
recompute age from timestamps and the activation `observed_at`.
Heartbeat dedup normalizes volatile provider-runtime timing and progress fields
before hashing, while keeping the full evidence in the diagnostic package for
`ccb_self`.

## Self Diagnostic Workflow

`ccb_self` should follow a fixed workflow so it is autonomous but predictable:

1. Read the suspicion envelope and classify which evidence is missing.
2. Gather cheap CCB state: `ps`, `trace`, `maintenance status`, comms queue or
   inbox state, and current config summary.
3. Gather provider-specific state only for the target provider.
4. Capture pane text read-only, then take a short activity sample if the first
   capture is ambiguous.
5. Use visual/screenshot fallback only when text cannot explain the state.
6. Decide one of:
   `healthy_now`, `watch_later`, `needs_user`, `repair_candidate`,
   `repair_applied`, or `cannot_decide`.
7. If no repair is safe, schedule a follow-up or report concise evidence.
8. If repair is safe, run the sanctioned CCB command, validate outcome, and
   schedule a follow-up.

## Detection Mechanism Shape

The classifier should be invariant-based rather than provider-rule-heavy:

- `state_conflict`: two authoritative-looking surfaces disagree.
- `progress_stale`: active work exists, but progress evidence is unchanged past
  a soft threshold and before hard timeout.
- `evidence_gap`: CCB cannot access the evidence needed for a confident
  decision.
- `provider_ambiguous`: provider-specific state says "maybe running" or "maybe
  not delivered" but hard failure criteria are not met.
- `recovery_needed`: a terminal or blocked state has a known CCB repair surface.

This gives `self` enough context while keeping ccbd efficient and conservative.

## Acceptance Criteria

- Healthy idle projects do not wake `ccb_self`.
- Known deterministic failures still terminalize or report through existing
  hard-authority paths.
- Gray-zone states wake `ccb_self` with bounded evidence and deduplication.
- `ccb_self` can diagnose a terminal-control-plane versus busy-pane conflict
  without requiring the user to manually inspect tmux.
- `ccb_self` can choose watch-later, ask-user, or bounded repair without
  creating duplicate business work.
