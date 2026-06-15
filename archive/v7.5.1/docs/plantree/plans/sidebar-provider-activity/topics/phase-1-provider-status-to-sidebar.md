# Phase 1 Provider Status To Sidebar

Date: 2026-05-27

Role: Implementation design
Status: Draft
Read when: Implementing provider-native status monitoring for sidebar agent
rows.

## Goal

Build the smallest provider-native status path that makes sidebar agent rows
reflect what the provider is actually doing.

This phase covers only:

```text
provider hook/session signal
  -> agent-scoped provider activity artifact
  -> ccbd project_view agent activity resolver
  -> ccb-agent-sidebar status symbol
```

This phase deliberately does not change mailbox, Comms, `ask`, reply capture,
retry, resubmit, callback, or message-bureau policy.

## Non-Goals

- Do not modify message-bureau state from provider activity.
- Do not retry, cancel, complete, or dead-letter jobs from provider activity.
- Do not make the Rust sidebar read provider files directly.
- Do not scan global tmux sessions or global provider homes.
- Do not store full prompts, full replies, API keys, or transcript content in
  the activity artifact.
- Do not require users to change external tmux or terminal settings.

## Authority And Precedence

Sidebar agent-row state should use this order:

```text
ccbd ownership/lifecycle guard
  -> provider-native activity evidence
  -> CCB queued-submit metadata when no provider evidence exists
  -> runtime health / pane liveness fallback
  -> pane text fallback
```

Meaning:

- namespace unmounted, stopped agent, failed reconcile, runtime fault, and dead
  pane still override provider activity;
- fresh provider activity is the execution-state authority for both manual pane
  turns and CCB-submitted turns;
- CCB job/message/Comms state is metadata for this phase, not execution-state
  authority;
- queued or accepted CCB work may show `pending` only while provider activity is
  absent;
- pane text is fallback evidence, not the primary signal when a valid provider
  artifact exists.

## Canonical Artifact Path

Use the existing runtime layout authority:

```text
PathLayout.agent_provider_runtime_dir(agent, provider) / "activity.json"
```

On disk this is normally:

```text
<runtime_state_root>/agents/<agent>/provider-runtime/<provider>/activity.json
```

Do not hard-code `.ccb/agents/...` because runtime state may be relocated away
from the project anchor.

## Activity Record

Minimal schema:

```json
{
  "schema_version": 1,
  "record_type": "provider_activity",
  "project_id": "project-id",
  "agent_name": "agent2",
  "provider": "codex",
  "state": "active",
  "source": "codex_hook",
  "event_name": "UserPromptSubmit",
  "ccb_session_id": "ccb-agent2-abc123",
  "runtime_dir": "/path/to/runtime/agents/agent2/provider-runtime/codex",
  "pane_id": "%42",
  "workspace_path": "/path/to/workspace",
  "provider_session_id": "optional",
  "provider_turn_id": "optional",
  "model": "optional",
  "updated_at": "2026-05-27T00:00:00Z",
  "diagnostics": {
    "tool_name": "optional",
    "error_type": "optional",
    "error_code": "optional",
    "error_message_preview": "optional"
  }
}
```

Allowed provider states:

| Provider state | ProjectView activity |
| :--- | :--- |
| `active` | `active` |
| `tool` | `active` |
| `waiting` | `pending` |
| `idle` | `idle` |
| `failed` | `failed` |

Provider-specific hook parsers may record compact diagnostics, but the shared
reader must expose only normalized state, source, reason, timestamp, and compact
diagnostics to `project_view`.

## Validation Rules

`project_view` must ignore an activity artifact when any hard identity check
fails:

- schema version is unsupported;
- `record_type != "provider_activity"`;
- `project_id` does not match the current project;
- `agent_name` does not match the current agent row;
- `provider` does not match the configured agent provider;
- `runtime_dir` does not match the current provider runtime directory;
- `ccb_session_id` is present and does not match the current runtime session id;
- `pane_id` is present and does not match the current runtime pane id;
- `workspace_path` is present and contradicts the current runtime workspace.

Freshness rules:

- `active`, `tool`, and `waiting` are trusted while the runtime session and pane
  identity still match.
- after a short soft-stale interval, pane text may downgrade `active` to `idle`
  only when it positively shows an idle prompt for the same live pane.
- after a longer hard-stale interval without terminal evidence, degrade
  `active/tool` to `pending` with reason `provider_activity_stale`, not `idle`.
- `failed` is sticky until the next provider `active/tool/waiting` event for
  the same runtime session, or until runtime ownership changes.
- runtime stop, pane death, project kill, or relaunch naturally invalidates old
  activity through identity checks.

## Provider Writers

Add a provider-activity hook helper separate from the existing completion hook:

```text
bin/ccb-provider-activity-hook
```

The helper should:

- read provider hook JSON from stdin;
- read `CCB_CALLER_ACTOR`, `CCB_CALLER_RUNTIME_DIR`, and `CCB_SESSION_ID` from
  the managed provider environment;
- read `TMUX_PANE` as best-effort pane identity;
- accept explicit `--provider`, `--project-id`, `--agent-name`,
  `--runtime-dir`, and `--workspace` args where available;
- write `activity.json` atomically;
- exit successfully on malformed input or write failure so provider execution is
  not blocked;
- never write full prompt or reply text.

Keep the existing `bin/ccb-provider-finish-hook` focused on completion
artifacts. Completion and activity are separate contracts.

## Provider Event Mapping

Codex initial mapping:

| Event | Activity |
| :--- | :--- |
| `SessionStart` | `idle` |
| `UserPromptSubmit` | `active` |
| `PreToolUse` | `tool` |
| `PostToolUse` | `active` |
| `PermissionRequest` | `waiting` |
| `Stop` | `idle` |
| provider/system error event | `failed` when payload is terminal, otherwise `waiting` |

Claude initial mapping:

| Event | Activity |
| :--- | :--- |
| `UserPromptSubmit` | `active` |
| `PreToolUse` | `tool` |
| `PostToolUse` | `active` |
| `Notification` | `waiting` only for user-attention payloads |
| `Stop` | `idle` unless background tasks are still running |
| API/system error payload | `failed` |

Gemini/OpenCode can stay on current fallback behavior in phase 1 unless their
hook payloads can be mapped without risk.

## ProjectView Integration

Add a small reader under `lib/ccbd/project_view/`, for example:

```text
lib/ccbd/project_view/provider_activity.py
```

Responsibilities:

- compute the activity path from `PathLayout` or an injected path layout;
- read at most one activity file per agent per `build_project_view()` call;
- validate identity and freshness against the current `AgentRuntime`;
- return a normalized `ProviderActivityEvidence` object;
- never mutate provider artifacts.

Extend `AgentActivityFacts` with optional provider activity evidence and update
`resolve_agent_activity()` so provider evidence is evaluated after lifecycle
guards and before CCB job/pane fallback.

When provider evidence is fresh and authoritative, `_agent_view()` should avoid
`capture-pane` unless the evidence is soft-stale and pane text is needed to
prove idle. This keeps sidebar refresh CPU bounded.

The current sidebar already renders `activity_state`, `activity_symbol`, and
`activity_color`. Rust changes should be limited to optional model fields or
tests unless the UI needs a new visible state.

## Test Slices

Automatic tests:

- writer writes atomic JSON for each normalized state;
- writer ignores malformed provider payload without non-zero exit;
- reader rejects wrong project, agent, provider, runtime dir, session id, pane,
  workspace, malformed JSON, unsupported schema, and future timestamp;
- `failed` stays failed when pane text shows idle;
- next provider turn clears sticky failed to active;
- fresh manual-provider `active` with no CCB job renders sidebar agent row as
  active;
- fresh provider `waiting` renders pending;
- fresh provider `idle` overrides stale CCB running metadata for the agent row
  without mutating job state;
- lifecycle guard still wins over provider activity;
- one ProjectView build reads each activity file at most once per agent;
- fresh provider activity does not trigger extra `capture-pane` calls.

Live smoke in `/home/bfly/yunwei/test_ccb2`:

- clean project and start CCB with at least one Codex and one Claude agent;
- manually type in a Codex pane and verify `idle -> active -> idle`;
- manually type in a Claude pane and verify `idle -> active -> idle`;
- trigger tool use and verify the agent does not appear idle during tool work;
- trigger a provider/API failure and verify `failed` remains visible until the
  next user/provider turn;
- run a CCB-submitted task and verify the same provider activity path drives the
  sidebar state.

## Release Gate For Phase 1

Phase 1 is done only when:

- Codex manual pane turns no longer show idle while running;
- Claude manual pane turns no longer show idle while running;
- provider failure is sticky until next provider turn or runtime ownership
  change;
- stale active evidence cannot keep an agent active forever after pane death or
  relaunch;
- sidebar still consumes only `project_view`;
- no mailbox/Comms/retry behavior changes are included in the patch.
