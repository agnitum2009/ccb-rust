# Ask Runtime Health Mechanism

Date: 2026-06-11

## Current Authority Chain

An ask task is judged by several layers with different authority:

- Dispatcher/job store owns authoritative job status:
  `accepted`, `queued`, `running`, `completed`, `cancelled`, `failed`, and
  `incomplete`.
- Provider execution owns provider-specific observation while the job is
  running. It sends prompts to the pane, polls provider logs/hooks/storage, and
  emits completion items or terminal decisions.
- Completion tracker turns completion items into provider-family-neutral state
  and terminal decisions.
- Job heartbeat is a long no-progress fuse for running `ask` jobs. It can
  terminalize as `incomplete/heartbeat_timeout` after repeated no-progress
  notices.
- Project view and maintenance heartbeat are diagnostics and escalation layers.
  They should not be treated as the completion authority.

## Running Detection

Submission:

- `ccb ask` submits a `MessageEnvelope` to the mounted daemon.
- The dispatcher creates one or more `JobRecord` rows and marks each as
  `accepted` or `queued` depending on whether the target already has
  outstanding work.
- On daemon ticks, the dispatcher starts the next runnable queued job, marks it
  `running`, records `job_started`, starts a completion tracker, marks the
  target active, and calls the provider execution adapter.

Provider execution:

- The execution service keeps an in-memory active submission per running job and
  persists recoverable execution state.
- Each ccbd polling cycle asks the provider adapter to poll its active
  submission.
- No update means the job remains running.
- Completion items update tracker state and progress timestamps.
- A terminal provider decision or terminal tracker decision completes the job.

Provider sources:

- Codex: protocol/session event stream under the managed Codex session root.
  Prompt delivery is not accepted until the active `CCB_REQ_ID` anchor appears
  in a valid protocol log. `task_complete` completes; `turn_aborted`, errors,
  pane death, or delivery-anchor failure terminate as non-success.
- Claude: hook artifact first when available, then session event log. The
  Claude hook must bind the current transcript turn to the actual CCB prompt and
  should emit nothing for unrelated provider-side turns.
- Gemini: hook artifact or session snapshot/result evidence.
- OpenCode: storage-backed assistant reply and completed message metadata.

## Fault Detection

Faults currently become terminal through these routes:

- startup/runtime binding failures: missing session, missing runtime context,
  pane unavailable, backend unavailable, or corrupt runtime state;
- pane liveness failure while polling active execution;
- provider-specific terminal errors such as Codex `turn_aborted`, Claude API
  errors, hook failure status, or OpenCode cancellation/failed completion;
- Codex prompt delivery failure when the anchor is still missing after the
  current log is drained, same-workspace fallback fails, and either the pane is
  unusable or the delivery timeout expires;
- provider no-terminal reliability timeout after no semantic progress for the
  provider policy window;
- generic completion tracker request timeout;
- job heartbeat timeout after repeated no-progress heartbeat notices for a
  running `ask` job.

Project-view fault symptoms:

- runtime/reconcile failure, missing unowned pane, provider terminal error
  markers, stale running job, prompt idle/input-stuck, callback wait, failed or
  incomplete comms, and delivery failure.

## Current Blind Spots

The present mechanism is strongest while the job remains active. It is weaker
after a false terminal decision because the active execution state is removed
and the agent is synced back to idle.

Known gaps:

- Project view captures pane text only when provider activity is absent or an
  active/pending provider activity needs a pane-error probe. That can miss
  terminal-control-plane versus busy-pane conflicts.
- Project view now exposes a bounded `provider_runtime` snapshot only when it
  matches the current active job, and exposes orphan/conflict summaries as
  evidence rather than authority. Maintenance heartbeat uses that evidence for
  `provider_runtime_without_control_job` and delayed
  `provider_delivery_pending_anchor` suspicion envelopes, but it still does
  not treat provider runtime state as completion authority.
- A terminal job with stale or wrong completion evidence may appear healthy
  unless the pane is independently sampled and conflicts with the terminal
  state.
- Provider runtime snapshots can still have short race windows around job
  terminalization or cleanup. Current dedup and pending-anchor timing gates are
  intended to absorb harmless drift, but snapshot string bounds and explicit
  stale-orphan policy remain follow-up hardening.
- `active` activity by itself is counted in the heartbeat summary but is not an
  issue; only failed, concerning pending, unknown pending, blocked, failed, or
  incomplete comms currently wake the assessor.
- Provider activity evidence can override CCB job evidence before pane text is
  sampled, which is useful for cheap status but risky for contradiction
  detection.

## Design Direction

Do not add a separate heartbeat mode. The default classifier should grow a
read-only active-anomaly pass that checks consistency between:

- authoritative job state;
- provider execution runtime state;
- provider-specific completion source state;
- pane text/activity sample;
- comms and callback state;
- maintenance activation state.

The pass should classify contradictions as `concern` or `unknown` and activate
the configured assessor through the existing bounded `ask --silence` path. It
must remain non-mutating in v1.
