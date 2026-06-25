# Workspace Shutdown

## Scenario: User-facing full workspace exit

### 1. Scope / Trigger

- Trigger: changes to `shutdown`, sidebar close behavior, daemon stop, or provider pane cleanup.
- User confirmation: the sidebar red `X` means complete `ccbr` workspace exit, not only closing the sidebar pane or stopping `ccbrd`.
- Hard rule: do not disable, remove, skip, or mask Codex hooks to make shutdown faster or cleaner.

### 2. Signatures

- RPC: `{"method":"shutdown","params":{}}`
- CLI: `ccbr shutdown`
- Daemon entry point: `CcbdApp::shutdown()`
- Stop flow: `CcbdApp::stop_all(force, reason)`

### 3. Contracts

- `shutdown` must request daemon shutdown and then run stop flow with `force=true`.
- Forced shutdown must terminate the managed tmux session, including the outer/root pane frame and all provider panes/processes for agents in the project namespace.
- Stale socket files may remain until cleanup, but no `ccbrd`, managed tmux session, Codex, Claude, or other provider process for the workspace may remain alive.
- `stop_all(force=false, ...)` remains the explicit non-forced internal stop path; do not use it for user-facing workspace exit.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Sidebar red `X` sends `shutdown` | daemon exits and managed tmux session plus agent panes/processes are gone |
| `ccbr shutdown` sends `shutdown` | same as sidebar red `X` |
| `shutdown` report is written | `actions_taken` includes `forced_cleanup` |
| Forced pane kill errors | report cleanup errors; do not silently claim all panes were killed |
| Codex hooks exist | leave them enabled; shutdown kills the process, not the hooks |

### 5. Good / Base / Bad Cases

- Good: before shutdown a managed tmux session and three agent panes are alive; after shutdown tmux reports no server/session and process scan shows no managed agent or daemon processes.
- Base: stopped daemon may leave socket files; test cleanup may remove them after verification.
- Bad: only `ccbrd` stops while Codex/Claude panes continue running.

### 6. Tests Required

- Unit: `CcbdApp::shutdown()` records `forced_cleanup` in the shutdown report.
- Integration/live smoke: start a workspace, call `ccbr shutdown`, then assert no managed provider/tmux/daemon processes remain and the project tmux socket reports no server/session.

### 7. Wrong vs Correct

#### Wrong

```text
shutdown -> stop_all(false, "shutdown")
```

#### Correct

```text
shutdown -> stop_all(true, "shutdown")
```
