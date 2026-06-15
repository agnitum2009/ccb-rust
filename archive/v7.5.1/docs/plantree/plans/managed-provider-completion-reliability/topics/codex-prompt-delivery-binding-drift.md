# Codex Prompt Delivery Binding Drift Repair

Date: 2026-06-12

Role: implementation plan
Status: planning
Authority: [managed provider completion reliability contract](../../../../managed-provider-completion-reliability-plan.md)
Read when: Codex pane-backed asks fail with `codex_prompt_delivery_failed`,
`delivery_anchor_missing`, or a worker appears idle/healthy while the Codex
session never records the active `CCB_REQ_ID`.
Related:
[roadmap](../roadmap.md),
[open questions](../open-questions.md),
[maintenance heartbeat plan](../../ccb-maintenance-heartbeat/README.md)

## Incident Boundary

The observed failure is not mailbox loss. The worker mailbox can receive and
consume the `task_request`, while Codex still never accepts the provider turn.

Confirmed boundary:

- `ccb trace` shows the message attempt and worker `task_request` were
  consumed.
- The reply terminalizes as failed with
  `reason = codex_prompt_delivery_failed`.
- Diagnostics show `delivery_failure_kind = delivery_anchor_missing`,
  `delivery_anchor_seen = false`, and a 120-second delivery timeout.
- The managed Codex session log and history do not contain the failed job's
  `CCB_REQ_ID`.

This means dispatcher `running` or mailbox `consumed` must not be treated as
provider acceptance. Codex acceptance requires the wrapped prompt anchor to
appear in a valid Codex protocol log.

## Failure Model

The concrete runtime shape is a soft-live Codex pane:

- tmux pane liveness and old Codex prompt text make the worker look usable;
- the recorded `codex.pid` or runtime PID can be dead or stale;
- the actual pane process can differ from recorded provider runtime facts;
- Codex activity and session log mtime can stop advancing before new asks;
- session identifiers can drift between the current CCB session file and the
  last provider activity record.

Current Codex delivery sends text to the pane through tmux paste plus Enter.
`wait_for_runtime_ready()` primarily samples pane content; it does not prove
the recorded Codex process, session log, activity hook, and pane process are
coherent. The delivery guard later catches the missing anchor, but only after
the timeout and after the user-visible ask has already failed.

## Goals

- Prevent a stale Codex pane/session binding from being reported as plain
  healthy when provider acceptance is not trustworthy.
- Refuse or degrade new Codex active submissions before sending a large prompt
  when local binding evidence is already stale.
- Convert `delivery_anchor_missing` into actionable health evidence for
  `ccb ps`, `doctor`, project view, maintenance heartbeat, and `ccb_self`.
- Keep retry behavior explicit until duplicate-execution risk is bounded.
- Preserve existing successful Codex pane-backed delivery behavior.

## Non-Goals

- Do not auto-resend a prompt solely because the anchor is absent.
- Do not treat mailbox consumption or tmux paste success as provider
  acceptance.
- Do not recover by raw tmux kill, manual pane mutation, or direct runtime-file
  edits.
- Do not broaden generic completion detector semantics in this slice.

## Immediate Operational Workaround

When this condition appears for an idle worker:

1. Stop sending large tasks to that worker.
2. Use the CCB control plane to replace only the affected agent:
   `ccb restart <agent>`.
3. Send a tiny smoke ask.
4. Verify a fresh `CCB_REQ_ID` appears in the managed Codex session log and the
   provider activity evidence advances before resubmitting important work.

This is a workaround, not the durable fix. It avoids trusting a pane that only
looks alive.

## Proposed Repair Slices

### Slice A: Codex Binding Evidence

Add bounded Codex binding facts that are evidence, not authority:

- recorded `codex.pid` value and liveness;
- bridge PID value and liveness;
- current tmux `#{pane_pid}` for the bound pane;
- whether recorded PID and current pane PID match;
- current session file, Codex session path, and protocol log path;
- session log mtime or size freshness for the active submission window;
- provider activity timestamp and CCB session id when available.

Expose this evidence through project view / doctor paths without changing
completion authority.

### Slice B: Delivery Preflight Gate

Before `start_active_submission()` sends a wrapped Codex prompt, run a Codex
binding preflight:

- pane must still exist;
- recorded/current pane process evidence must be coherent, or explicitly
  classified as uncertain;
- bridge and communication artifacts must be present when the configured mode
  expects them;
- the selected session log must be readable and consistent with the session
  file;
- stale activity/log evidence should degrade the runtime instead of silently
  proceeding.

If preflight fails, return a retryable provider-runtime error with diagnostics
such as `provider_binding_stale` or `codex_binding_unhealthy`. Do not paste the
prompt.

### Slice C: Delivery Failure Classification

When the existing delivery guard reaches `delivery_anchor_missing`, persist and
surface the condition as provider health evidence:

- mark the active provider runtime as degraded for the affected agent;
- include checked session root, current log path, pane id, PID facts, and
  activity freshness in diagnostics;
- make `ccb ps`, `doctor`, or maintenance heartbeat distinguish
  "mailbox consumed but provider did not accept anchor" from ordinary idle;
- recommend guarded restart for the affected agent.

This slice should not imply the prompt never executed. It only states that CCB
lacks provider-acceptance evidence.

### Slice D: Guarded Restart And Retry Policy

Keep the first implementation operator-driven:

- `delivery_anchor_missing` is retryable but not automatically retried;
- `ccb_self` or the user may run `ccb restart <agent>` when the agent is idle
  and no anchor/reply evidence exists;
- a later automatic recovery policy can be considered only after duplicate
  side-effect risk is bounded.

Future automatic retry is only eligible when all of these hold:

- no anchor was observed in current or fallback logs;
- no reply started;
- the failure happened before provider acceptance;
- the runtime was replaced cleanly through `ccb restart`;
- the caller explicitly requested retry or the job is proven idempotent.

### Slice E: Regression Coverage

Add focused tests before runtime validation:

- dead `codex.pid` with alive pane is reported as stale binding evidence;
- current pane PID mismatch is visible in health/project-view output;
- Codex active start preflight refuses to paste when binding evidence is stale;
- healthy Codex binding still sends prompt and enters `pending_anchor`;
- `delivery_anchor_missing` includes binding diagnostics and retryable status;
- maintenance heartbeat flags stale or long-pending prompt delivery without
  becoming completion authority;
- `ccb ps` / doctor rendering exposes the degraded condition without claiming
  mailbox loss.

## Acceptance Criteria

- A worker with stale Codex PID/session evidence no longer appears as only
  healthy idle when recent prompt delivery failed.
- New Codex asks do not paste large prompts into a runtime that preflight
  already knows is stale.
- `trace` and reply diagnostics still preserve mailbox lineage and make clear
  that the mailbox event was consumed.
- Operators get an explicit, safe recovery path: restart the affected agent,
  run a smoke ask, then retry intentionally.
- Existing healthy Codex task delivery and anchor detection continue to pass.

## Verification Path

Targeted unit/regression tests:

- `test/test_codex_comm_session_runtime.py`
- `test/test_stability_regressions.py`
- `test/test_ccbd_project_view.py`
- `test/test_maintenance_heartbeat.py`
- provider execution service tests that cover active submission snapshots

Source runtime validation must follow project isolation rules:

```bash
/home/bfly/yunwei/ccb_source/ccb_test --diagnose
cd /home/bfly/yunwei/test_ccb2
HOME=/home/bfly/yunwei/test_ccb2/source_home \
CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home \
/home/bfly/yunwei/ccb_source/ccb_test <scenario>
```

Manual validation scenario:

1. Start a managed Codex worker.
2. Confirm a smoke ask records `CCB_REQ_ID` in the Codex session log.
3. Simulate or fixture stale `codex.pid` / mismatched pane PID evidence.
4. Confirm preflight blocks or degrades before prompt paste.
5. Confirm `ccb restart <agent>` restores smoke ask delivery.

## Rollback Notes

Health evidence exposure is low risk because it is observational. The higher
risk change is the preflight gate: if it blocks valid Codex sessions, rollback
should disable the gate while keeping diagnostics and the existing
`delivery_anchor_missing` guard.
