# Provider Structured Readback

## Scenario: provider completion readback and pane fallback isolation

### 1. Scope / Trigger

- Trigger: any change to provider polling, Claude/Codex JSONL readers, daemon pane capture, inbox reply extraction, or provider runtime-state keys that carry completion text.
- Reference owners:
  - Claude project logs: `~/.claude/projects/<project-key>/*.jsonl` where `<project-key>` preserves a leading slash as `-` (for example `/mnt/d/dapro-ass` -> `-mnt-d-dapro-ass`).
  - Codex session JSONL: the named `.ccbr/.codex-<agent>-session` binding.
- Runtime owner: `ccbrd` provider execution plus `ccbr-providers` polling.
- Hard rule: do not disable, remove, skip, or mask Codex hooks to make readback easier.

### 2. Signatures

- Daemon active-pane capture state key: `pane_text_buffer`.
- Structured reply state keys: `reply_buffer`, `last_agent_message`, provider-specific log reader state such as `session_path`, `claude_projects_root`, `state.log_path`, and `state.offset`.
- Claude project key function: map every non-ASCII-alphanumeric character to `-`; do not trim leading or trailing dashes.
- Provider polling output: terminal `ProviderPollDecision` with clean final reply text, not raw pane text.

### 3. Contracts

- `reply_buffer` is reserved for provider-owned structured reply text. Daemon tmux capture must never overwrite it.
- Daemon tmux capture writes only `pane_text_buffer` for best-effort fallback detection.
- If a structured log binding exists (`session_path`, `claude_projects_root`, or provider-specific log path), provider polling must prefer that log and must not terminalize from pane text.
- Pane fallback is allowed only when no structured log source is expected or available for that submission.
- Inbox-visible replies must not contain original prompts, TUI chrome, progress spinners, hook output, or terminal prompt decorations.
- When a completion-tracker decision terminalizes a job, provider execution state must also be finished so active-only polling stops.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Claude project root configured | Poll Claude JSONL under the exact project key; do not trim the leading `-` |
| Structured log exists but has no terminal assistant event | Keep job running; do not complete from `pane_text_buffer` |
| Structured log has final assistant text | Complete with that text, ignoring dirty pane text |
| No structured log binding exists | Pane fallback may complete only after provider-specific prompt/anchor readiness checks |
| Daemon captures pane text | Runtime state updates `pane_text_buffer`, not `reply_buffer` |
| Completion tracker returns terminal decision | Remove the job from active provider execution state |

### 5. Good / Base / Bad Cases

- Good: Claude writes `CCBR_SMOKE_SUBMIT_*` to its JSONL and the sender inbox shows exactly that token.
- Base: pane text contains `Reply exactly: TOKEN` plus a spinner while JSONL is still pending; the job remains running.
- Bad: inbox reply includes the original prompt, provider TUI chrome, or startup/hook text because pane capture overwrote `reply_buffer`.

### 6. Tests Required

- Unit: Claude project key preserves leading dash for absolute project paths.
- Unit: daemon active-pane capture stores `pane_text_buffer` separately from provider `reply_buffer`.
- Unit: Claude pane fallback returns `None` when structured logs are expected.
- Unit: structured Claude completion ignores dirty `pane_text_buffer` and returns `last_agent_message` / final assistant text.
- Unit: Codex pane fallback reads `pane_text_buffer` first, with legacy `reply_buffer` only as compatibility fallback.
- Live smoke: `ccbr ask <target> --from <sender> "Reply exactly: TOKEN"` then `ccbr inbox --detail <sender>` contains only `TOKEN`.

### 7. Wrong vs Correct

#### Wrong

```text
feed_active_pane_text_to_execution -> runtime_state["reply_buffer"] = capture-pane text
provider poll -> terminal reply from reply_buffer while structured logs are configured
```

#### Correct

```text
feed_active_pane_text_to_execution -> runtime_state["pane_text_buffer"] = capture-pane text
provider poll -> structured JSONL final assistant text wins; pane fallback only when no structured log source is expected
```
