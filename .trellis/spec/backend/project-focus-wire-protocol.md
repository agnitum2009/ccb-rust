# Project Focus Wire Protocol

## Scenario: sidebar requests tmux focus changes through ccbrd

### 1. Scope / Trigger

- Trigger: any change to `project_focus_window`, `project_focus_agent`, ProjectView namespace epoch handling, sidebar focus retry, or tmux pane/window selection.
- Reference owner: Python `backup/python-reference/lib/ccbd/project_focus/service.py`.
- Consumer owner: Rust `tools/ccb-agent-sidebar/src/tui.rs`.

### 2. Signatures

- RPC op: `project_focus_window`
- Payload: `{ "window": "<window name>", "namespace_epoch": <optional u64> }`
- Response: `{ "focused": true, "kind": "window", "window": "<window>", "agent": <first agent|null>, "namespace_epoch": <u64> }`
- RPC op: `project_focus_agent`
- Payload: `{ "agent": "<agent name>", "namespace_epoch": <optional u64> }`
- Response: `{ "focused": true, "kind": "agent", "window": "<window>", "agent": "<agent>", "namespace_epoch": <u64> }`

### 3. Contracts

- Focus handlers must execute tmux focus, not only acknowledge the request.
- If `namespace_epoch` is supplied, it must equal the mounted namespace epoch; stale values return an error containing `stale_view`.
- `project_focus_window` selects the target tmux window.
- For agent windows, `project_focus_window` also selects the first configured agent pane when that pane is known.
- Tool windows have no agent pane and only select the window.
- `project_focus_agent` resolves the configured window, requires a known pane id, selects the window, then selects the pane.
- After successful focus, sidebar refresh is best-effort; refresh failure must not fail the focus request.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Missing namespace | Error contains `target_missing` |
| Invalid agent/window name | Error contains `invalid_request` |
| Unknown window | Error contains `unknown_window` |
| Unknown agent | Error contains `unknown_agent` |
| Stale namespace epoch | Error contains `stale_view` |
| Agent pane missing | Error contains `target_missing` |
| tmux select-window fails | Error contains `target_missing` |
| tmux select-pane fails | Error contains `tmux_focus_failed` |

### 5. Good / Base / Bad Cases

- Good: clicking an agent row focuses the configured window and the exact pane for that agent, then the sidebar refreshes ProjectView.
- Base: clicking a tool window focuses only the tool window and leaves agent pane selection alone.
- Bad: the handler returns `status=ok` while tmux focus remains unchanged.

### 6. Tests Required

- Unit: `focus_agent_plans_window_and_pane_selection`.
- Unit: `focus_tool_window_does_not_select_agent_pane`.
- Unit: `focus_rejects_stale_namespace_epoch`.
- Package: `cargo test -p ccbr-daemon -- --test-threads=1`.

### 7. Wrong vs Correct

#### Wrong

```json
{ "status": "ok", "agent": "agent2", "namespace_epoch": 4 }
```

#### Correct

```text
select-window -t ccbr-session:main
select-pane -t %2
```

```json
{ "focused": true, "kind": "agent", "window": "main", "agent": "agent2", "namespace_epoch": 4 }
```
