# Current Ccbd Comms And Retry

Date: 2026-05-27

Role: Current-facts topic
Status: Reference
Read when: Changing sidebar/provider activity, job retry, ProjectView Comms,
or dispatcher recovery behavior.

## Purpose

This note records the current `ccbd` communication-state and retry behavior so
future provider-activity work does not accidentally treat existing Comms/job
state as provider execution truth.

## Current Comms Status Path

`project_view` builds Comms rows in `lib/ccbd/project_view/service.py`:

```text
active/queued jobs
  + bounded recent job tail
  + message-bureau attempt/reply metadata
  + reply delivery source folding
  -> latest business job per message/agent lineage
  -> business_status/status_label
```

Important behavior:

- Reply-delivery jobs are not business Comms rows. They are folded into the
  source business job when possible.
- Multiple retry attempts for the same message/agent lineage collapse to the
  latest attempt row.
- `business_status` is a communication workflow label, not a provider
  execution-state label.
- `running_recover_hint` is currently inferred from pane text heuristics for
  Codex/Claude running jobs and can mark a Comms row as `blocked/stuck`.
- Comms recoverability is attached to rows through
  `comms_recoverability_for_job()`.

Current row status mapping:

| Job condition | Comms business status |
| :--- | :--- |
| accepted/queued | `sending` |
| running | `replying` |
| completed, reply delivery pending | `delivering` |
| completed, reply delivered | `replied` |
| reply delivery failed | `delivery_failed` |
| cancelled | `cancelled` |
| incomplete | `incomplete` |
| failed | `failed` |

## Message Bureau Lineage

`record_submission()` creates one `MessageRecord`, one `AttemptRecord` per
target agent, and one queued inbound `TASK_REQUEST` per attempt.

Default retry policy attached to new messages:

```json
{
  "mode": "auto",
  "max_attempts": 3,
  "retryable_reasons": ["api_error", "transport_error"],
  "retry_runtime_when_resume_supported": true,
  "retryable_runtime_reasons": ["pane_dead", "pane_unavailable"]
}
```

Terminal completion records:

- update the attempt state from the terminal job status;
- consume or abandon the inbound task request;
- refresh the message state from latest attempts and replies;
- record a `ReplyRecord`;
- optionally queue a reply delivery back to the sender.

## Automatic Retry Path

Current automatic retry happens after a job reaches a terminal
`CompletionDecision`:

```text
complete_job()
  -> persist_terminal_completion()
  -> record_message_bureau_completion()
  -> schedule_automatic_retry()
  -> automatic_retry_plan()
  -> dispatcher.retry(current.job_id)
```

`automatic_retry_plan()` requires:

- the job has an attempt record;
- the parent message has `retry_policy.mode = "auto"`;
- the terminal decision is `failed`;
- the failure is retryable by reason or error type;
- retry count has not reached `max_attempts`.

Retryable by default:

- `reason = api_error`
- `reason = transport_error`
- `diagnostics.error_type` in `api_error`, `transport_error`,
  `provider_api_error`
- runtime reasons such as `pane_dead` or `pane_unavailable` only when the
  provider supports resume and policy allows runtime retry

Non-retryable API failures short-circuit retry:

- authentication/login errors
- permission/access errors
- quota/billing errors

These are detected from `decision.reason`, `diagnostics.error_type`,
`diagnostics.error_code`, and selected error-message markers.

## Manual Retry Path

`dispatcher.retry(target)` resolves a terminal latest attempt and creates a new
job under the same message lineage.

Rules:

- active attempts cannot be retried;
- completed attempts cannot be retried;
- only the latest attempt for the same message/agent can be retried;
- the target agent must still be available;
- retry attempts get a new job id and a new attempt id;
- `retry_source_job_id` points back to the job that caused the retry.

Retry body behavior:

- if the previous attempt entered context (`anchor_seen` or `reply_started`),
  retry sends body `continue`;
- otherwise retry replays the original request body;
- `retry_delivery_mode = continue` is persisted in provider options when using
  the continue path.

## Comms Recoverability

`comms_recoverability_for_job()` can mark a Comms row recoverable when:

- reply delivery failed and the reply-delivery job can be retried;
- the source business job is terminal failed/cancelled/incomplete and can be
  retried;
- a running job appears stale by runtime health, pane state, or a trusted
  running hint.

Trusted running hints are currently:

- `provider_prompt_idle`
- `provider_prompt_idle_stale`
- `provider_prompt_input_stuck`
- `job_running_stale`

Recovering stale running jobs currently cancels the old job without recording a
reply, then retries the job.

## Implications For Provider Activity

Do not use current Comms status as sidebar execution truth.

Reasons:

- CCB messages are delivered by simulating provider user input, so a manual turn
  and a CCB-managed turn should share the same provider-native activity path.
- A CCB job can be `running` while the provider has already failed, returned to
  idle, or is waiting for user action.
- A provider can be actively working with no CCB job when the user typed
  directly in the pane.
- Comms row state is about message workflow and reply delivery, not agent
  execution state.

The future activity resolver should instead use:

```text
ccbd ownership/lifecycle guard
  -> provider-native execution state
  -> pane/runtime fallback
```

CCB job/message/attempt records remain useful metadata for lineage, retry,
reply delivery, and diagnostics, but they should not be the primary agent-row
activity authority.

## Testing Notes

When provider-native activity lands, keep focused regression tests for:

- Comms lineage still collapses retries correctly;
- auto retry still schedules only after terminal failed decisions;
- non-retryable auth/permission/billing failures do not auto-retry;
- sticky provider `failed` state does not force Comms retry by itself;
- provider activity `active/idle/failed` does not mutate message-bureau state
  unless a real job terminalization path emits a terminal decision.
