# Codex Wire Protocol

## Scenario: CCBR ask delivery into Codex

### 1. Scope / Trigger

- Trigger: any change to `ccbr ask` delivery, Codex session discovery, Codex JSONL polling, inbox reply extraction, or pane fallback completion.
- Hard rule: do not disable, remove, skip, or mask Codex hooks, including `session_start`. Preserve hooks and solve coordination through launch args, developer instructions, session binding, and protocol polling.

### 2. Signatures

- CLI: `ccbr ask <target> --from <sender> <message>`
- Session files: `.ccbr/.codex-<agent>-session` for named Codex agents; do not fall back to `.ccbr/.codex-session` for named agents.
- Isolated live-smoke roots that launch real Codex must carry the project `.codex/hooks` files; a smoke without hooks is invalid evidence, not a successful hook-enabled provider proof.
- Prompt state key consumed by daemon ask delivery: `runtime_state["prompt_text"]`.
- Prompt dispatch owner for Python-style `submit`: Codex provider execution `start`, not the Rust-only `ask` handler.
- Provider state keys: `session_path`, `state.log_path`, `state.offset`, `request_anchor`, `anchor_seen`, `reply_buffer`, `last_agent_message`.

### 3. Contracts

- Wrapped Codex prompts contain `<<BEGIN:req-xxxxxxxx>>` followed by the user body.
- Python-style `submit` must drive the same provider-owned prompt dispatch path as Python CCB: when heartbeat promotes a queued Codex job to running, provider execution start wraps and sends the prompt.
- Rust-only `ask` is a convenience endpoint; if provider execution already records `prompt_sent=true`, `ask` must not send the same prompt again.
- If Codex startup consumes an early paste before a session event exists, active delivery may resend the same prompt once after the pane shows the ready prompt and before the current `request_anchor` is observed.
- Codex session payloads used for pane dispatch must carry the workspace tmux socket path when a custom tmux socket is used.
- Codex JSONL is authoritative when `session_path` points to an existing file.
- `event_msg` with `payload.type = "user_message"` confirms anchor delivery when the text contains the exact `request_anchor`.
- `event_msg` with `payload.type = "agent_message"` or `response_item` assistant messages may supply reply text.
- `event_msg` with `payload.type = "task_complete"` is the terminal boundary; prefer `last_agent_message` for the final reply.
- Pane text fallback is allowed only when no structured Codex JSONL file exists and the pane buffer contains the current job `request_anchor`; a ready prompt without the anchor is startup/TUI noise, not turn completion.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Named agent session file missing | submission stays unavailable/error; no primary-session fallback |
| Isolated smoke root lacks `.codex/hooks` | reject the smoke as invalid; do not treat hook-blocked output as provider parity evidence |
| `prompt_text` missing | daemon cannot deliver wrapped prompt; treat as protocol bug |
| `submit` heartbeat starts Codex job | provider start sends one wrapped prompt to the target pane |
| Prompt sent before Codex TUI is ready and anchor remains unseen | resend once after the pane shows ready prompt; mark `prompt_resent_after_ready=true` |
| provider start records `prompt_sent=true` | Rust-only `ask` reports delivered without duplicate pane send |
| JSONL exists but has no new terminal event | keep job running; do not complete from pane text |
| JSONL contains `task_complete.last_agent_message` | complete job and store that text in sender inbox |
| No JSONL file exists and pane contains current `request_anchor` | pane fallback may detect a ready prompt as best-effort completion |
| Pane shows ready prompt but lacks current `request_anchor` | keep job running; do not deliver TUI/startup text as the reply |

### 5. Good / Base / Bad Cases

- Good: `agent1 -> agent2` ask completes and `ccbr inbox --detail agent1` shows only `agent2`'s final answer.
- Base: startup keeps Codex hooks enabled and injects CCBR coordination through launch `developer_instructions`.
- Bad: inbox preview contains Codex TUI chrome, the original prompt, or hook warning text.

### 6. Tests Required

- Unit: named `.ccbr/.codex-<agent>-session` is chosen and `prompt_text` equals the wrapped prompt.
- Unit: provider `start` sends the wrapped prompt to a tmux-backed Codex session and records `prompt_sent=true`.
- Unit: pending-anchor polling resends once after Codex ready prompt when the current anchor is still unseen.
- Unit: daemon `submit` + heartbeat sends the wrapped prompt through provider execution.
- Unit: request anchor matching accepts the actual `<<BEGIN:req-...>>` text.
- Unit: when JSONL exists, pane fallback must not complete before `task_complete`.
- Unit: pane fallback with Codex startup warning and `›` ready prompt but no current `request_anchor` returns incomplete.
- Live smoke: `ccbr ask agent2 --from agent1 "Reply exactly: TOKEN"` then `ccbr inbox --detail agent1` contains `TOKEN`.
- Live smoke: for real Codex provider completion, assert the pane shows `UserPromptSubmit hook (completed)`, `ccbr trace <job>` shows `completed`, and `inbox <agent> --detail` returns `pending=0`.

### 7. Wrong vs Correct

#### Wrong

```text
Codex pane shows a ready prompt, so complete the job from captured pane text.
```

#### Correct

```text
If Codex JSONL exists, wait for `task_complete` and use structured final-answer text.
```

#### Wrong

```text
Create an isolated smoke root without `.codex/hooks`, then accept a blocked `UserPromptSubmit` run as provider evidence.
```

#### Wrong

```text
Pane contains `›`, so deliver the whole pane buffer as the reply even though the current request anchor never appeared.
```

#### Correct

```text
Copy or otherwise preserve `.codex/hooks` in the smoke root, then require `UserPromptSubmit hook (completed)` plus trace/inbox proof.
```

#### Correct

```text
Pane fallback waits for the current `request_anchor` before treating `›` as this turn boundary.
```

#### Correct

```text
Pending-anchor delivery may resend once after Codex becomes ready, then waits for JSONL/task completion.
```
