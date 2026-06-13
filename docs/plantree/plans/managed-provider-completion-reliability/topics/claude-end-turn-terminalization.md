# Claude End-Turn Terminalization

Date: 2026-06-12

## Incident

Claude-backed `ask` jobs can visibly finish in the provider session and emit a
completion item like:

```text
kind = assistant_chunk
provider = claude
text / merged_text = non-empty assistant reply
stop_reason = end_turn
```

The following completion state can still remain non-terminal:

```text
anchor_seen = true
reply_started = true
reply_stable = false
tool_active = false
terminal = false
```

After the 900-second execution reliability timeout, the caller receives
`status=incomplete`, `reason=completion_timeout`, even though a usable provider
reply was already visible.

## Code Evidence

- Claude structured parsing already extracts `message.stop_reason`.
- Claude assistant chunk payload already includes `stop_reason`.
- Claude assistant state handling currently emits `TURN_BOUNDARY` only when the
  raw buffer contains `CCB_DONE`.
- Claude system event handling also emits `TURN_BOUNDARY` for
  `system/turn_duration` when it matches the last assistant UUID.
- `SessionBoundaryDetector` records `ASSISTANT_CHUNK` as pending and only
  terminalizes on `TURN_BOUNDARY`.

Therefore `stop_reason=end_turn` is preserved as evidence, but no component
currently promotes it to the normalized terminal boundary that CCB expects.

## Accepted First Slice

Fix this in the Claude provider state machine, not in generic completion
detectors.

Claude-specific rule:

- if the current event is a primary assistant event, not a subagent event;
- `stop_reason == "end_turn"`;
- the cleaned assistant text or merged reply buffer is non-empty;
- the active request anchor has been observed;
- and no turn boundary has already been emitted;

then append `CompletionItemKind.TURN_BOUNDARY` with:

- `reason = "assistant_end_turn"`;
- `last_agent_message` from the current merged reply;
- `turn_id`;
- `session_path`;
- `assistant_uuid`;
- `stop_reason = "end_turn"`.

Do not interpret `tool_use`, `pause_turn`, `max_tokens`, or `stop_sequence` as
normal completed turns in this first slice.

## Empty Boundary Guard

`SessionBoundaryDetector` should match the existing `ProtocolTurnDetector`
empty-boundary policy:

- `TURN_BOUNDARY` with a reply, or with prior assistant-visible reply evidence,
  can complete normally.
- `TURN_BOUNDARY` with no reply and no prior `reply_started` evidence must
  terminalize as `incomplete/task_complete_empty_reply`.
- Diagnostics should include `empty_reply = true` and
  `error_type = "empty_provider_reply"`.

This prevents session-boundary providers from reporting normal completion when
a provider terminal event carries no assistant-visible answer.

## Test Matrix

Required focused tests:

- Claude assistant text with `stop_reason=end_turn`, no `CCB_DONE`, produces
  `ASSISTANT_CHUNK` then `TURN_BOUNDARY`.
- Claude subagent `stop_reason=end_turn` does not mark the parent request
  terminal.
- Claude `stop_reason=tool_use` remains pending.
- Claude `stop_reason=end_turn` with empty text does not complete normally.
- Claude already-completed `CCB_DONE` or `turn_duration` paths do not emit
  duplicate boundaries.
- `SessionBoundaryDetector` empty `TURN_BOUNDARY` becomes
  `incomplete/task_complete_empty_reply`.
- A tracker/dispatcher integration test proves the job completes before
  `completion_timeout` and reply delivery uses the normal completed path.
- Claude `end_turn` that is intentionally not accepted as terminal, such as a
  subagent event or missing anchor case, still falls back to the 900-second
  execution reliability timeout with correct diagnostics.
- After accepted Claude `end_turn` terminalization, the generic completion
  tracker timeout must not later emit a second terminal decision.
- Silence job with accepted Claude `end_turn`: the job terminalizes normally
  while successful reply delivery remains suppressed.
- Callback parent flow: a child Claude `end_turn` completion submits the
  callback continuation with the expected reply routing.
- Session rotation before the active turn resets stale detector state without
  preventing a later anchored Claude `end_turn` from terminalizing correctly.

Recommended adjacent regression tests:

- Claude hook transcript attribution.
- Claude finish hook script behavior.
- Provider execution service runtime tests.
- Completion tracker and orchestration tests.
- Silence and callback chain tests that already cover successful and abnormal
  child completion.

## Risks And Guardrails

- Silence jobs may terminalize earlier, but successful silence reply delivery
  should remain suppressed by dispatcher policy.
- Callback chains may resume earlier, which is the expected result when Claude
  has actually ended the turn.
- Subagent events must not end the parent job.
- Tool-use turns must remain pending until provider evidence shows the tool
  turn has resolved.
- Empty provider replies must remain diagnosable as incomplete.

## Evidence References

- Worker analysis artifact:
  `.ccb/ccbd/artifacts/text/completion-reply/job_80bfbee17b78-art_d8d33e2fb575474f.txt`
- Ask-system reviewer artifact:
  `.ccb/ccbd/artifacts/text/completion-reply/job_ce2e44a10cfe-art_7c2d55013e0f4131.txt`
