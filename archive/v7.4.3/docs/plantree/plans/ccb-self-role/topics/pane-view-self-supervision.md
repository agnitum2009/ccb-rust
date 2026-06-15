# Pane View Self Supervision

Date: 2026-06-10

## Direction

`ccb_self` self-supervision should inspect the real CCB-owned pane view when
diagnosing ambiguous agent execution state. The v1 primary mechanism should be
tmux text capture, especially:

- the bottom of the pane, where provider status, active prompt, warnings,
  update prompts, quota messages, and current input usually appear;
- recent scrollback around the current request;
- short-interval capture comparison to see whether output is changing.

Queue, trace, ps, and doctor output remain control-plane evidence, but they
often cannot prove whether a provider pane is really working, waiting for
input, showing a stale prompt, displaying a provider update, or stuck on an
error. Screenshots should be a fallback for cases where text capture is
insufficient or a layout/visual problem matters.

## Evidence Ladder

Use this order for ambiguous runtime progress:

1. control-plane snapshot to identify target agent, active job, queue state,
   and expected pane;
2. read-only pane metadata from the current CCB tmux namespace;
3. `tmux capture-pane` text capture from the target pane, biased toward the
   bottom/current prompt region;
4. short activity sampling by comparing captures over a bounded interval;
5. bounded CCB-owned pane screenshot or equivalent visual artifact only when
   text capture cannot classify the state;
6. semantic assessment by `ccb_self`;
7. schedule follow-up, report, or user escalation through CCB control-plane
   policy.

This is pane-view-first for execution-quality diagnosis, not authority. The
mounted daemon graph, lifecycle, runtime records, queue, inbox, and trace still
define configured-agent authority and job lineage.

## Why Pane Text View Matters

The recent `archi` incident showed the gap:

- queue/trace said an ask was `running` or `delivering`;
- `ccb ps` could report a bound/alive pane;
- provider logs showed a Codex update and pane death;
- `tmux capture-pane` showed whether the new request reached the provider,
  whether the pane had moved past an old prompt, and whether work continued
  after context compaction.

The safest repair came from combining lineage with the live pane text: cancel
the stale head of line, let the valid next job run, avoid restart while the pane
was visibly working, then cancel duplicate retries after success.

## Scope

In scope:

- configured CCB agent panes;
- CCB sidebar panes;
- configured managed tool windows;
- text capture of current pane content and recent scrollback;
- bottom-region/current-prompt inspection;
- comparison of consecutive pane captures to classify progress;
- screenshot fallback for CCB-owned panes/windows when text is insufficient.

Out of scope:

- arbitrary desktop screenshots;
- unrelated tmux sessions or browser tabs;
- screenshots of provider auth or secret material when avoidable;
- using pane text or screenshots to define live configured agents, restart
  targets, or job lineage;
- mutating tmux through observation tools.

## Supervision Inputs

The self-supervision prompt should receive references and short summaries, not
large raw dumps:

- target agent and active job/message/attempt ids;
- reason the heartbeat or user diagnostic could not classify progress;
- queue/inbox/trace summary;
- pane id, window, workspace, provider, runtime state, and capture timestamp;
- bottom pane text summary or artifact reference;
- recent scrollback artifact reference when needed;
- last two observation fingerprints when comparing progress;
- screenshot artifact reference only when text capture is insufficient;
- recent terminal/provider failure summaries;
- whether captured text or screenshot may contain sensitive UI so `ccb_self`
  should avoid quoting raw content.

## Observation Result

`ccb_self` should return a compact structured result:

- `health`: `healthy`, `concern`, `failing`, or `unknown`;
- `confidence`: `high`, `medium`, or `low`;
- `pane_state`: `working`, `waiting_input`, `stale_prompt`,
  `provider_update`, `provider_error`, `dead_or_blank`, `misframed`,
  `unknown`;
- `pane_capture`: references to bottom/current-prompt capture and optional
  scrollback/activity samples;
- `visual_evidence`: optional screenshot/OCR artifact references when used;
- `recommended_action`: `report_only`, `schedule_followup`, or `ask_user` in
  v1;
- `next_heartbeat_after`: optional bounded delay with reason;
- `needs_user`: true when the pane appears to require manual provider input,
  may expose sensitive UI, or cannot be repaired autonomously.

Mutating repairs stay outside v1 self-supervision unless a separate autonomous
repair policy is accepted.

## Tooling Requirements

Minimum v1 tool surfaces:

- `ccb_pane_capture_text`: bounded `tmux capture-pane` text capture for a
  configured CCB target, with options for bottom/current screen and scrollback
  depth.
- `ccb_pane_activity_sample`: compare pane text/metadata across a short
  interval.

Fallback tool surfaces:

- `ccb_pane_screenshot`: capture a CCB-owned pane/window/sidebar/tool target
  to a CCB-owned artifact path when text is insufficient.
- `ccb_visual_inspect`: summarize a screenshot artifact with OCR or vision
  when available.

The role should not depend on arbitrary desktop capture APIs.

## Heartbeat Interaction

The heartbeat runner should not capture pane contents or screenshots on every
tick. It should escalate to `ccb_self` with a reason such as
`progress_unknown`, `active_job_old`, `provider_status_conflict`, or
`pane_unusable_marker`, plus target references.

Allowed flow:

1. heartbeat detects ambiguous or unhealthy execution state;
2. heartbeat dispatches `ask --silence` to `ccb_self` with bounded snapshot and
   target references;
3. `ccb_self` uses sanctioned read-only pane observation tools, starting with
   `tmux capture-pane` style text capture;
4. `ccb_self` uses screenshot fallback only if text evidence cannot classify
   the visible state;
5. `ccb_self` schedules a follow-up or asks the user when the pane state is
   still ambiguous.

This keeps the heartbeat cheap and independent while letting `ccb_self` inspect
what the agent actually sees.

## Safety Rules

- Capture only CCB-owned panes/windows/tool targets resolved from current CCB
  authority.
- Store capture artifacts under project-owned CCB artifact storage.
- Prefer text capture before screenshot; use screenshot only when text capture
  is unavailable, blank, misleading, or insufficient for a visual/layout
  failure.
- Do not quote large pane text or sensitive UI content in replies.
- Do not infer authority from pane text or pixels. Reconcile pane evidence with
  trace, queue, runtime, lifecycle, and config evidence.
- Do not use observation tools to send keys, click, focus, resize, or mutate
  panes.

## V1 Acceptance Criteria

- `ccb_self` has a documented pane-view self-supervision skill or runbook.
- Heartbeat assessor contract passes enough target references for `ccb_self`
  to request text capture without the runner reading pane contents itself.
- A stuck-provider incident can be classified from trace + bottom pane capture
  + activity sample without raw tmux mutation.
- Screenshot fallback is available or explicitly planned for cases text capture
  cannot classify.
- Unknown pane states shorten cadence only through bounded schedule-followup
  policy and cap repeated unknowns.
