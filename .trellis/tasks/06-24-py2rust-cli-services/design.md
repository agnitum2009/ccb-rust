# Design: py2rust-cli-services-parity

## Architecture and Boundaries

All six clusters live inside `rust/crates/ccb-cli`. The crate is a client of
`ccb-daemon` and the provider/terminal/storage crates; it does not own the
control plane. Work must stay inside `ccb-cli` and its tests unless a clear
utility belongs in `ccb-provider-core` or `ccb-storage`.

### Per-cluster boundaries

| Cluster | Primary files | Allowed external touches |
|---------|---------------|--------------------------|
| `cleanup_service` | `src/services/cleanup.rs`, `tests/cleanup_service_tests.rs` | None expected |
| `diagnostics_bundle` | `src/services/diagnostics_runtime/*.rs`, `tests/v2_diagnostics_bundle_tests.rs` | None expected |
| `kill_runtime_agent_cleanup` | `src/services/kill_runtime/agent_cleanup.rs`, `tests/kill_runtime_agent_cleanup_tests.rs` | None expected |
| `doctor_runtime` | `src/services/doctor.rs`, `src/services/doctor_runtime/*.rs`, `tests/doctor_runtime_identity_tests.rs` | `render.rs` / ops views for command wiring |
| `kill_service` | `src/services/kill.rs`, `src/services/kill_runtime/*.rs`, `tests/kill_service_tests.rs` | `src/services/daemon.rs` for daemon client helpers |
| `ask_service` | `src/services/ask.rs`, `src/services/ask_runtime/*.rs`, `src/ask_sender.rs`, `tests/ask_service_tests.rs` | `src/services/daemon.rs` for client helpers only |

## Data Flow and Contracts

### `cleanup_service`

- Input: `CleanupOptions` (project root, dry run, force, max age/count).
- Output: `CleanupSummary` with deleted/skipped counts and per-category logs.
- Existing `CleanupService` trait allows deterministic tests; extend tests only.

### `diagnostics_bundle`

- Input: project root, optional output path, optional runtime root.
- Output: tarball path + manifest JSON.
- Source collector already respects provider-state exclusion lists; extend
  tests for edge cases rather than changing the exclusion contract.

### `kill_runtime_agent_cleanup`

- Input: project paths, configured/extra agent names, force flag, control-plane
  PID candidates.
- Output: `LocalShutdownState` with tmux sockets, agent runtimes, and merged
  PID candidates.
- Add `force=true` and missing-runtime-file edge-case tests.

### `doctor_runtime`

- New `doctor.rs` orchestrator should assemble:
  1. Runtime identity summary (`doctor_runtime/system.rs`).
  2. CCBD state summary (daemon connection / project focus).
  3. Agents layout/role summary (from `ccb-agents` crate if available).
  4. Any warnings from stores.
- Render through existing doctor ops views; do not invent a new render contract.

### `kill_service`

- Existing stage order in `kill_project_with`:
  1. Stop maintenance heartbeat.
  2. Collect authority PID snapshot.
  3. Request remote stop.
  4. Prepare local shutdown.
  5. Destroy namespace.
  6. Resolve summary.
  7. Finalize.
- Missing pieces:
  - `record_shutdown_intent` must persist a `kill_intent.jsonl` event before
    remote stop.
  - `shutdown_daemon` must terminate the ccbd PID, clear its lease, and guard
    against replaced lease holders.
  - `record_kill_report` must write the shutdown report store after remote
    stop.
  - Start-policy files must be cleared.
  - Runtime PID files (`bridge.pid`, `codex.pid`, helper-manifest leader PID)
    must be terminated.

### `ask_service`

- Stay within existing `submit_ask_with` and `write_ask_output` contracts.
- Add deterministic tests for:
  - Legacy role alias `ccb.archi` → `agentroles.archi`.
  - `exit_code_for_ask_status` and `write_ask_output` newline handling.
  - Relocated runtime actor resolution (`CODEX_RUNTIME_DIR`).
- Escalate before modifying `invoke_mounted_daemon` retry/reconnect or
  `watch_ask_job` reconnect semantics.

## Compatibility and Migration Notes

- All new behavior must match the Python reference tests in `test/`. No
  breaking changes to existing `ccb-cli` command-line interfaces.
- New files under `kill_runtime/` should follow the existing module naming
  (`remote.rs`, `finalize.rs`, `pid_cleanup.rs`, `agent_cleanup.rs`).
- Doctor command wiring should reuse existing `ops_views_doctor` render paths.

## Operational / Rollback Considerations

- `kill_service` changes affect project shutdown; tests must run against
  deterministic fakes, never against a live tmux server or daemon.
- `cleanup_service` dry-run tests must remain side-effect-free.
- If any change accidentally alters provider-state exclusion in diagnostics
  bundle, it could leak credentials into support tarballs; add an explicit
  regression test for the auth/cache exclusion list.

## Important Trade-offs

- **Completeness vs. escalation**: We will fully implement and test the
  non-socket surfaces of `ask_service`, but stop and report if the Python
  parity requires changing the daemon socket retry contract.
- **Single task vs. child tasks**: This task keeps all six clusters under one
  plan because they share the same crate and the same validation command.
  Implementation will still proceed cluster-by-cluster.
