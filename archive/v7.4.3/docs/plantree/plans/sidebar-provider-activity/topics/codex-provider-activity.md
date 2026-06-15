# Codex Provider Activity

## 1. Purpose

This plan defines how CCB should integrate Codex-native lifecycle hooks into
`project_view` so the sidebar can show manual Codex activity accurately.

The immediate trigger is a sidebar false-idle case: a user starts work directly
inside a managed Codex pane, no CCB job is active, and `project_view` reports
`idle` because it only sees runtime liveness plus pane text heuristics.

The target is not to import `tmux-agent-status` as a tmux plugin. The target is
to adapt its hook-status idea into CCB's existing authority model.

## 2. Upstream Reference

External project:

- `samleeney/tmux-agent-status`
- inspected snapshot: `efefbed`
- relevant files:
  - `hooks/codex-hook.sh`
  - `hooks/better-hook.sh`
  - `scripts/lib/session-status.sh`
  - `scripts/lib/collect.sh`
  - `scripts/status-line.sh`
  - `tests/codex-hook-lifecycle.sh`
  - `tests/status-line-codex-regression.sh`

Useful upstream behavior:

- Codex state is hook-driven through `SessionStart`, `UserPromptSubmit`,
  `PreToolUse`, `PostToolUse`, and `Stop`.
- Hooks write small status files, then a collector renders status from those
  files.
- Hook-managed pane state overrides process polling.
- `UserPromptSubmit` marks work active and clears wait/park overrides.
- `Stop` marks work done.
- Legacy process polling exists only as a bootstrap fallback for Codex sessions
  without hook state.

Upstream parts CCB should not import:

- global tmux session scanning
- TPM plugin lifecycle
- user-global `~/.cache/tmux-agent-status`
- generic wait/park/switcher UX
- sidebar rendering or status-line rendering
- cross-session aggregation outside the current CCB project namespace
- direct tmux focus or pane mutation from the sidebar process

## 3. CCB Boundary Decision

The sidebar remains a `project_view` client.

Codex hooks may enrich provider activity, but they must not become project,
agent, pane, job, or layout authority. The authoritative path remains:

```text
Codex hook -> agent-scoped CCB activity file -> ccbd project_view ->
ccb-agent-sidebar
```

The Rust sidebar must not read hook files directly.

## 4. Status Artifact

Add an agent-scoped, provider-owned activity artifact under the managed Codex
runtime/state boundary. Candidate path:

```text
.ccb/agents/<agent>/provider-runtime/codex/activity.json
```

The path should be produced by a provider runtime artifact helper rather than
hard-coded in hook scripts, following the runtime layout direction in
`docs/managed-provider-completion-reliability-plan.md`.

Minimal record shape:

```json
{
  "schema_version": 1,
  "provider": "codex",
  "source": "codex_hook",
  "agent": "agent2",
  "state": "active",
  "event": "UserPromptSubmit",
  "session_id": "...",
  "turn_id": "...",
  "pane_id": "%2",
  "cwd": "/path/to/project",
  "model": "gpt-5.5",
  "updated_at": "2026-05-27T00:00:00Z"
}
```

Allowed `state` values should map into existing ProjectView states:

- `active` -> `activity_state=active`
- `tool` -> `activity_state=active`
- `waiting` -> `activity_state=pending`
- `idle` -> `activity_state=idle`
- `failed` -> `activity_state=failed`

The file is advisory evidence. It must be ignored when it conflicts with the
current configured agent generation, live pane identity, or provider family.

## 5. Hook Event Mapping

Managed Codex home materialization should install a CCB-owned hook command.

Suggested mapping:

- `SessionStart`
  - write `idle`
  - clear stale turn fields when source is startup, resume, or clear
- `UserPromptSubmit`
  - write `active`
  - include `turn_id`, `session_id`, `prompt` hash or length only, and `cwd`
  - do not store full prompt text in the activity file
- `PreToolUse`
  - write `tool`
  - include tool name when available
- `PermissionRequest`
  - write `waiting`
  - used to avoid showing a blocked agent as idle
- `PostToolUse`
  - write `active`
  - include tool result status when available
- `Stop`
  - write `idle`
  - preserve terminal turn id and timestamp

Claude can later use the same CCB activity-file shape, but this plan focuses on
Codex because the reported false-idle case is Codex manual pane work.

## 6. Managed Codex Configuration

CCB must configure hooks inside the managed Codex home only.

Requirements:

- do not mutate the user's global `~/.codex/config.toml`
- do not depend on a user-installed tmux plugin
- do not require a user to run `/hooks` and trust CCB hooks manually when the
  hook can be installed as managed CCB startup authority
- if Codex requires `features.hooks = true`, set it only in the managed home
- if Codex supports `codex --enable hooks`, prefer the managed config route
  unless launch-time enablement is more stable for the current Codex version
- inherited user hook config must not be allowed to override or remove the CCB
  managed activity hook

Open question:

- Whether current Codex treats CCB-projected hooks as managed/trusted by default
  or still requires an explicit trust record in the managed home.

## 7. ProjectView Resolution

`project_view` should resolve activity in this order:

1. active/queued CCB job state
2. provider-native activity file for the current agent generation
3. runtime health and pane liveness
4. pane text fallback

Provider-native activity must be freshness-bounded:

- if the artifact is older than a short TTL and the pane is alive with an idle
  prompt, return `idle`
- if the artifact says `active` but the Codex pane is dead, return `offline` or
  `failed` based on existing runtime health
- if no `Stop` arrives because Codex crashed, avoid permanently stuck `active`
  by using pane liveness and mtime expiry

This fixes both known failure modes:

- historical scrollback should not make an idle agent pending
- manual Codex work should not look idle only because no CCB job is active

## 8. Process Polling Fallback

`tmux-agent-status` has a Codex fallback that walks the process tree and treats
child subprocesses below the deepest Codex runner as active work.

CCB should not make that the primary path because it is provider-version and
platform sensitive. It may be useful as a diagnostic fallback only:

- disabled by default or used only when hooks are unavailable
- bounded to the project tmux socket and configured agent pane
- never scans global tmux sessions
- never overrides a fresh hook artifact

## 9. Privacy And Storage

The activity artifact must avoid storing full prompts, replies, or transcript
content. It should store identifiers, timestamps, event names, and compact
diagnostics only.

Diagnostic bundles may include recent activity artifacts because they are
project-local runtime evidence, but the bundle classifier should treat them as
runtime status, not provider conversation history.

## 10. Tests

Focused tests:

- managed Codex home materialization installs the CCB activity hook without
  mutating global Codex config
- hook script `UserPromptSubmit` writes `active`
- hook script `PreToolUse` writes `tool`
- hook script `PermissionRequest` writes `waiting`
- hook script `Stop` writes `idle`
- malformed hook stdin exits successfully and writes a degraded diagnostic
- `project_view` uses fresh provider activity to mark a no-job Codex agent
  `active`
- `project_view` uses fresh provider waiting to mark a no-job Codex agent
  `pending`
- stale provider activity does not keep the agent active forever
- provider activity for the wrong agent, pane, generation, or provider is ignored
- CCB running job state still wins over provider artifact state

Live smoke in `/home/bfly/yunwei/test_ccb2`:

- start CCB with a Codex agent
- submit a manual prompt in the Codex pane
- verify sidebar changes from idle to active without a CCB job
- let the turn finish
- verify sidebar returns to idle
- interrupt with Escape or `ccb cancel agent`
- verify no permanently stuck active state remains

The full validation matrix, including dedicated API lanes, provider disconnects,
invalid auth, unavailable model, hook failures, and manual smoke testing, is
tracked in [test-matrix.md](test-matrix.md).

## 11. Implementation Phases

### P0 Plan And Probe

- document upstream findings and CCB boundary decisions
- build a throwaway hook script in a test project to confirm current Codex hook
  trust and payload behavior
- record whether `features.hooks = true` is sufficient in managed homes

### P1 Codex Activity Hook

- add CCB-owned hook script or small Python helper
- materialize managed Codex hook config during provider-home preparation
- write agent-scoped `activity.json`
- add focused hook tests

### P2 ProjectView Integration

- add a provider activity reader under `lib/ccbd/project_view/` or provider
  runtime utilities
- merge activity evidence into the existing resolver with explicit precedence
- add stale/freshness tests and no-job manual-work tests

### P3 Cross-Provider Shape

- decide whether Claude should emit the same `activity.json` shape
- integrate Claude background task nuance from `tmux-agent-status`
- keep provider-specific hook parsing provider-owned
- apply the shared validation matrix in [test-matrix.md](test-matrix.md)

### P4 App-Server Evaluation

- separately evaluate Codex `app-server` as a future backend transport
- do not block the hook-based sidebar fix on that larger architecture change

## 12. Risks

- Codex hooks are still described upstream as experimental, so CCB must fail
  closed to pane/runtime fallback if hooks are missing.
- Hook trust behavior may differ between global, project-local, plugin, and
  managed-home hooks.
- A hook script that blocks could delay Codex turns, so writes must be local,
  fast, and best-effort.
- Freshness expiry that is too short can flicker active->idle during long model
  thinking; expiry must account for turns that produce no tool calls.
- Freshness expiry that is too long can keep crashed turns active.
- Provider activity must not reintroduce the earlier ProjectView CPU issue by
  scanning large logs or global tmux state every second.
