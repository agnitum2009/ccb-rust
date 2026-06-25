# Project Clear Context Wire Protocol

## Scenario: sidebar/CLI requests provider context clearing

### 1. Scope / Trigger

- Trigger: any change to `project_clear_context`, agent pane binding, provider clear/reset commands, or sidebar clear-context action routing.
- Reference owner: Python `backup/python-reference/lib/ccbd/handlers/project_clear.py`.
- Runtime owner: Rust `ccbrd` project namespace plus agent registry.

### 2. Signatures

- RPC op: `project_clear_context`
- Payload: `{ "agent_names": ["agent1", "agent2"] }`; empty or omitted targets all configured agents.
- Response: `{ "status": "ok", "agent_names": [...], "results": [...] }`.

### 3. Contracts

- Handler must send `/clear` to real provider panes; it must not return an empty successful no-op.
- Empty target list means all configured agents.
- Target `"all"` means all configured agents and cannot be combined with named agents.
- Named targets are normalized, de-duplicated, and rejected if unknown.
- For each target, resolve the current pane from project namespace first, then registry pane binding.
- If the namespace is not mounted, fail loudly instead of claiming success.
- For each live pane, leave copy mode best-effort, send `C-u`, literal `/clear`, then `Enter`.
- For OpenCode agents, wait 300ms before submitting `Enter` to avoid dropped immediate submit after session restore.
- Result rows use Python-compatible states:
  - `cleared` with `pane_id` and `command: "/clear"`.
  - `skipped` with `reason: "runtime_missing"` or `"pane_missing"`.
  - `failed` with truncated `reason` and `pane_id`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| `agent_names` omitted | Clear all configured agents |
| Duplicate target names | Clear each agent once |
| `"all"` plus another name | Error contains `cannot be combined` |
| Unknown target | Error contains `unknown agent` |
| Namespace missing | Error contains `project namespace is not mounted` |
| Pane id missing or invalid | Result `skipped/pane_missing` |
| Pane no longer exists | Result `skipped/pane_missing` with `pane_id` |
| tmux send fails | Result `failed` with truncated reason |

### 5. Good / Base / Bad Cases

- Good: selecting clear context for all agents sends the `/clear` sequence into every configured live agent pane and reports per-agent rows.
- Base: a stopped/missing pane is reported as skipped without failing the whole request.
- Bad: response says `status=ok` with `results=[]` while no provider pane receives `/clear`.

### 6. Tests Required

- Unit: `project_clear_context_targets_all_agent_panes_with_provider_clear`.
- Unit: `project_clear_context_dedupes_requested_agents_and_rejects_unknown`.
- Unit: `project_clear_context_reports_missing_panes`.
- Package: `cargo test -p ccbr-daemon -- --test-threads=1`.

### 7. Wrong vs Correct

#### Wrong

```json
{ "status": "ok", "agent_names": ["all"], "results": [] }
```

#### Correct

```text
send-keys -t %1 C-u
send-keys -t %1 -l /clear
send-keys -t %1 Enter
```

```json
{
  "status": "ok",
  "agent_names": ["agent1"],
  "results": [{"agent": "agent1", "status": "cleared", "pane_id": "%1", "command": "/clear"}]
}
```
