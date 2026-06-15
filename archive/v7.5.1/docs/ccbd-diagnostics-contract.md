# CCBD Diagnostics Contract

## 1. Purpose

This document defines the non-drifting diagnostics contract for project-scoped startup/shutdown reports, backend logs, and support-bundle export in `ccb_source`.

It is the authoritative design anchor for:

- `.ccb/ccbd/startup-report.json`
- `.ccb/ccbd/shutdown-report.json`
- `.ccb/ccbd/state.json`
- `.ccb/ccbd/start-policy.json`
- `.ccb/ccbd/lifecycle.jsonl`
- `.ccb/ccbd/heartbeats/<subject-kind>/*.json`
- `.ccb/ccbd/maintenance-heartbeat/`
- `.ccb/ccbd/reload-drain.json`
- `.ccb/ccbd/artifacts/text/`
- project-scoped backend log retention under `.ccb/ccbd/`
- `ccb doctor`
- `ccb doctor ps`
- `ccb doctor logs <agent>`
- `ccb doctor --bundle`

The repo-local memory file [AGENTS.md](/home/bfly/yunwei/ccb_source/AGENTS.md) must point to this document instead of duplicating the rules.

## 2. Goals

Diagnostics must let another user reproduce backend state and failure context without interactive shell access to the original machine.

That means the diagnostics surface must answer at least:

- what the project anchor was
- which config was active
- whether the backend was mounted, restarted, recovered, or shut down
- which agents were expected, mounted, degraded, or stopped
- what the daemon and keeper most recently logged
- which authority files and event streams existed at the time of export

## 3. Hard Contract

### 3.1 Project Scope

- Diagnostics are scoped to one `.ccb` anchor.
- All project diagnostics records must live under that anchor's logical `.ccb/ccbd/`, even when the physical runtime state root is relocated on WSL mounted-drive projects.
- Runtime-root marker and reference files are part of the project diagnostics evidence chain and must be mapped back into the logical `.ccb` archive path in support bundles.
- Project-local provider state under `.ccb/agents/<agent>/provider-state/` is diagnostics evidence and should be exported when it is relevant to session isolation or binding analysis.
- Diagnostics export must never merge multiple project anchors into one bundle.

### 3.2 Startup Report

Path:

- `.ccb/ccbd/startup-report.json`

The latest startup report must capture the most recent startup-related transaction, including:

- `trigger`
  - at minimum `daemon_boot` or `start_command`
- `status`
  - at minimum `ok` or `failed`
- `generated_at`
- `daemon_generation`
- optional `daemon_started`
  - whether the foreground `ccb` command had to start a new daemon
- `requested_agents`
- `desired_agents`
- `actions_taken`
- `agent_results`
- `inspection`
- `socket_placement`
  - at minimum preferred/effective socket paths plus root kind and fallback reason for both `ccbd` and project tmux socket selection
- optional `failure_reason`

Rules:

- daemon boot must write a startup report
- foreground `start` must overwrite it with the more specific `start_command` report
- startup report write failure must not replace the original startup error with a diagnostics-only error
- when project tmux preparation fails, `failure_reason` must preserve the user-facing startup failure plus tmux command context, the effective tmux socket path, socket path byte length when known, and original tmux stderr/stdout detail when available

### 3.3 Shutdown Report

Path:

- `.ccb/ccbd/shutdown-report.json`

The latest shutdown report must capture the most recent shutdown-related transaction, including:

- `trigger`
  - at minimum `shutdown`, `stop_all`, `kill`, or `kill_fallback`
- `status`
- `generated_at`
- `forced`
- `stopped_agents`
- `actions_taken`
- `cleanup_summaries`
- `inspection_after`
- optional `failure_reason`

Rules:

- normal server-side stop/shutdown must write a shutdown report
- CLI fallback kill must also write a shutdown report
- the final persisted shutdown report must reflect post-shutdown state, not an intermediate pre-unmount snapshot
- remote `ccb kill` must finalize lifecycle state before recording the final shutdown report, so `inspection_after` reflects `phase=unmounted` / `desired_state=stopped` rather than a transient `stopping` state

### 3.4 Backend Logs

Project backend logs must remain under `.ccb/ccbd/`:

- `ccbd.stdout.log`
- `ccbd.stderr.log`
- `keeper.stdout.log`
- `keeper.stderr.log`

Rules:

- daemon and keeper must append logs to stable file paths
- diagnostics readers must treat these as evidence, not authority
- large logs may be tailed during export, but the manifest must explicitly mark truncation

### 3.5 Namespace State And Lifecycle

Paths:

- `.ccb/ccbd/state.json`
- `.ccb/ccbd/start-policy.json`
- `.ccb/ccbd/lifecycle.jsonl`
- `.ccb/ccbd/heartbeats/<subject-kind>/*.json`
- `.ccb/ccbd/maintenance-heartbeat/schedule.json`
- `.ccb/ccbd/maintenance-heartbeat/status.json`
- `.ccb/ccbd/maintenance-heartbeat/runner.json`
- `.ccb/ccbd/maintenance-heartbeat/lock.json`
- `.ccb/ccbd/maintenance-heartbeat/activations.jsonl`
- `.ccb/ccbd/reload-drain.json`

Rules:

- `state.json` records the latest persisted project tmux namespace facts
- `start-policy.json` records the persisted project recovery startup policy, including inherited `auto_permission` and forced recovery-restore semantics
- `lifecycle.jsonl` records namespace creation/destruction and later runtime lifecycle events
- `heartbeats/<subject-kind>/*.json` records non-lease heartbeat state for long-lived supervised subjects such as running jobs; these files are diagnostics/evidence, not backend ownership authority
- `maintenance-heartbeat/` records CCB maintenance heartbeat schedule, status,
  runner, lock, and activation evidence. It is a project-scoped maintenance
  namespace, not daemon lease heartbeat authority and not subject/job heartbeat
  evidence.
  - `schedule.json` records the next maintenance heartbeat time and reason
    when a heartbeat tick or later schedule command updates cadence.
  - `status.json` records the latest maintenance heartbeat status summary from
    `ccb maintenance tick`, including `last_tick_status`, `last_tick_at`,
    `last_ok_at`, `unknown_streak`, `source_kind`, `recommended_action`,
    `next_heartbeat_after_s`, `needs_user`, last activation fields, a bounded
    `summary`, and bounded `evidence`.
  - `runner.json` records best-effort schedule consumer diagnostics, including
    `runner_id`, `pid`, `state`, `source`, `started_at`, `last_seen_at`,
    `last_wake_at`, `last_tick_at`, `last_tick_status`,
    `observed_next_run_at`, `sleep_until`, and `exit_reason`. It is diagnostic
    state for the project-scoped helper and is not daemon lifecycle authority.
  - `lock.json` records best-effort heartbeat operation lock metadata. The
    lock is independent from keeper, lease, and daemon lifecycle locks.
  - `activations.jsonl` records `ActivationIntent` dispatch outcomes such as
    `submitted`, `suppressed`, `blocked`, or `failed`. These records are
    diagnostics/audit evidence for CCB-originated silent asks and are not
    mailbox, job, or daemon authority.
  - malformed or missing files must be visible to `ccb maintenance status`
    without crashing the read path.
  - status readers may report `missing` or `corrupt`; they must not repair,
    rewrite, or migrate these files as a side effect.
  - `ccb maintenance tick` may write only `status.json`, `schedule.json`,
    `lock.json`, and `activations.jsonl` in this namespace. The project-scoped
    schedule consumer may write only `runner.json` and invoke the same one-shot
    tick path when the schedule is due. When non-healthy evidence requires
    semantic supervision, the tick may submit one silent ask to the configured
    assessor through the mounted daemon dispatcher, then exit. Neither tick nor
    runner may write provider state, run repairs, or mutate daemon lifecycle
    authority.
- `reload-drain.json` records bounded pending unload/replace drain state when
  explicit reload state machinery is invoked; it is not lifecycle, lease,
  runtime authority, or a config-watch trigger
- `reload-handoff.json` records a short-lived explicit reload handoff while
  plain `ccb reload` submits a non-dry-run request and, after daemon-side plan
  acceptance, while additive apply with a changed config signature is in
  progress; it lets keeper distinguish a bounded
  old-signature-to-target-signature transition from daemon drift. It is cleared
  after apply, is not lifecycle/lease/runtime/namespace authority, is not a
  config-watch trigger, and stale or mismatched records fail closed.
- `artifacts/text/` stores oversized CCB agent-to-agent message and reply text. Request bodies, terminal replies, notices, and callback continuations larger than 4 KiB are written there as UTF-8 text artifacts; ledgers store only the short preview plus artifact path, byte count, and sha256 metadata. These artifacts are diagnostics/evidence and transport support, not scheduling authority.
- running-job heartbeat observations stay in diagnostics/events and must not be emitted as caller-visible mailbox replies; after three consecutive no-progress observations, the terminal `heartbeat_timeout` reply is the caller-visible outcome
- daemon lease heartbeat, subject/job heartbeat, and maintenance heartbeat
  schedule/status/activation state must remain separate concepts and separate
  files
- `doctor` and bundle export must include these records when present
- `ping('ccbd')` and `doctor` should surface start-policy summary fields when available
- `ping('ccbd')` and `doctor` must surface namespace summary fields such as epoch, tmux socket path, session name, and latest lifecycle event when available
- `ping('ccbd')` and `doctor` must surface current socket placement diagnostics, including preferred/effective socket path, root kind, fallback reason, and filesystem hint when known
- `ping('ccbd')` and `doctor` should surface lightweight control-plane metrics when available, including handler latency, heartbeat wall duration, heartbeat step duration, heartbeat runtime-store write count, project-view cache/response/build/tmux counts, process RSS/FD/thread counts, service-graph version/created-at/retained-count metadata, pending maintenance ticks, and reload timing fields when a reload feature exists; until old-graph in-flight retention is implemented, `service_graph_retained_count` means published graph count, not RCU-style old graph retention; these metrics are diagnostics only and must not add config watchers, tmux mutations, or heavy steady-state scans
- `ccb reload --dry-run` / `project_reload_config(dry_run=true)` is a diagnostics-grade planning surface: it validates the current `.ccb/ccb.config`, returns old/new config signatures, plan class, no-mutation `safe_to_apply=false`, future classification safety, operations, optional drain intent suggestions for unload/replace, reasons, warnings, and errors, and updates reload timing fields without mutating tmux/runtime/lifecycle/service graph
- `ccb reload` / `project_reload_config(dry_run=false)` is an explicit additive apply surface: `no_change` returns `status=noop` without graph publish, and only `view_only_change`, `maintenance_change`, append-only `add_agent`, `add_window`, idle `remove_agent`, `add_tool_window`, and `remove_tool_window` may publish. `maintenance_change` publishes a new service graph/config signature without tmux namespace mutation, runtime mount/unload, or agent pane restart. Non-additive `replace_agent`, `move_agent`, unsupported tool changes, and arbitrary `layout_change` must stop before graph publish and report structured diagnostics. Successful apply diagnostics must include stage, graph versions, publish flags, keeper handoff safety, and project-view cache invalidation state. Failure diagnostics must preserve stage-specific namespace/runtime residue while leaving the old published graph/config visible.
- heartbeat wall duration may include lifecycle wrapper work outside the named step map; project-view build duration is updated only on cache misses, while project-view response duration is updated on cache hits and misses
- process RSS, virtual memory, FD count, and thread count are best-effort process metrics; platforms without Linux `/proc` support may report `None`
- `doctor` must also surface preferred/effective socket path byte lengths and an equivalent isolated-config `tmux -f /dev/null -S <effective-socket> start-server` command when a project tmux socket path is known, so macOS and WSL socket pathname failures can be diagnosed from one report
- malformed namespace diagnostics must surface as diagnostics errors, not silently disappear
- supervision diagnostics must preserve mount-attempt distinctions:
  - `mount_started` details should include `mount_attempt_id` when present
  - superseded finalize paths should remain visible as `mount_superseded`
    instead of collapsing into missing history

### 3.6 Doctor Read Path

`ccb doctor` is the best-effort project diagnostics read path.

Rules:

- it must summarize current backend inspection plus latest persisted reports
- `doctor ps` and `doctor logs <agent>` are converged diagnostics subviews of
  the same diagnostics surface
- if top-level `ps` and `logs` are retained, they must remain compatibility
  entrypoints over the same diagnostics meaning rather than drifting into a
  second independent diagnostics surface
- it should surface current mailbox summary authority for configured agents when
  present, including at minimum summary version/source/freshness plus head and
  queue facts needed to diagnose summary-vs-ledger drift
- it should also surface mailbox summary consistency status for configured
  agents by comparing persisted summary authority against a diagnostics-grade
  ledger projection without mutating mailbox artifacts
- missing, unreadable, or drifted mailbox summaries must remain visible in
  doctor output as explicit consistency mismatch/error state rather than being
  silently repaired during the read path
- agent binding diagnostics must include both `tmux_socket_name` and `tmux_socket_path` when known so project-scoped namespace bugs can be diagnosed from logs alone
- startup failure diagnostics must retain chained cause detail in CLI output and in `ccbd_startup_last_failure_reason` when the backend recorded it
- Codex agent diagnostics should surface managed in-pane session-switch state
  when `.ccb/agents/<agent>/provider-runtime/codex/session-switch.json`
  exists, including state, reason, commit status, and candidate session
  identity
- `doctor storage` must surface the effective `shared_cache_root`,
  `shared_cache_root_usable`, and shared-cache status/reason, so WSL relocation
  and future provider cache sharing decisions are diagnosable from the same
  storage view. When the reason is `wsl_drvfs_requires_runtime_relocation`, the
  root must be reported as unavailable instead of pointing at an unsafe drvfs
  path. The current disabled reason code is
  `wsl_drvfs_requires_runtime_relocation`; relocated WSL projects should report
  shared cache as enabled.
- it must not crash only because one diagnostics artifact is missing or malformed
- malformed diagnostics files must surface as diagnostics errors, not silent omission

### 3.7 Support Bundle Export

Command:

- `ccb doctor --bundle`

Default output location:

- `.ccb/ccbd/support/<bundle-id>.tar.gz`

The support bundle must include:

- a manifest
- a generated doctor snapshot
- current project config from `.ccb/ccb.config`
- latest lifecycle reports
- backend authority files such as lease, keeper, shutdown intent, and namespace state when present
- backend recovery policy authority such as `start-policy.json` when present
- persisted non-lease heartbeat state under `.ccb/ccbd/heartbeats/` when present
- maintenance heartbeat schedule/status/runner/lock/activation files under
  `.ccb/ccbd/maintenance-heartbeat/` when present
- oversized CCB text artifacts under `.ccb/ccbd/artifacts/text/` when referenced by recent message/reply records
- recent backend event streams such as supervision, namespace lifecycle, and cleanup history
- backend stdout/stderr logs
- per-agent runtime authority and recent agent/provider logs
- non-secret project-local provider-state evidence such as managed Codex homes, session roots, session logs, and config overlays
- a generated storage classification snapshot at
  `generated/storage-summary.json`
- relevant external session files when discoverable from runtime authority

Rules:

- bundle export must be best-effort and continue when some files are missing or malformed
- manifest rows must include original source path, archive path, inclusion status, and truncation status
- bundle export must not require the backend to be healthy
- bundle export must be project-local and deterministic enough for support usage
- provider-state export must exclude credential material such as copied auth
  tokens and provider-managed credential files like `auth.json` or
  `oauth_creds.json`; Gemini projected auth artifacts such as `.env` and
  `google_accounts.json` must also be excluded
- provider-state export must use the storage classification model from
  [docs/ccb-provider-state-storage-boundary-plan.md](/home/bfly/yunwei/ccb_source/docs/ccb-provider-state-storage-boundary-plan.md)
  to exclude `SECRET`, `REBUILDABLE_CACHE`, and
  `STARTUP_AUTHORITY_BUNDLE` payload files from the archive while preserving
  their path/class/size metadata in `generated/storage-summary.json`
- if storage classification fails, provider-state export must still apply a
  conservative path hard-filter for known cache/startup-bundle directories such
  as Codex `.tmp/plugins/`, Claude `.local/share/claude/versions/`, and
  Gemini/npm rebuildable cache paths; classification failure must not turn
  excluded payloads into archive entries
- provider-state export must not follow symlinks while walking provider-state
  trees
- Codex managed-home violations must remain visible as diagnostics evidence; bundle export must not hide them by silently replacing the managed reader source with global `~/.codex/sessions`

### 3.8 Keeper Child Reaping

The keeper may directly spawn `ccbd`, but it must reap exited direct children.

Rule:

- a crashed or killed `ccbd` process must not remain visible as an unreaped zombie just because keeper is still alive

## 4. Operational Workflow

Recommended support workflow:

1. reproduce the issue in the project anchor
2. run `ccb doctor`
3. run `ccb doctor --bundle`
4. send the generated tarball

The bundle is the transport unit. The reports inside it are the authoritative timeline.

## 5. Update Discipline

- If startup or shutdown reporting changes, update this document in the same patch.
- If `doctor` or bundle contents change materially, update this document in the same patch.
- Use [docs/ccbd-manual-test-issue-log.md](/home/bfly/yunwei/ccb_source/docs/ccbd-manual-test-issue-log.md) for concrete incidents and repro findings.
