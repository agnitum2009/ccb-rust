# Rust Functional Parity Owner Plan — Python 7.5.2 → 7.7.0 Upgrade

## Boundary correction

Current practical target is no longer "stop at Python `v7.5.2`". The target is:

```text
Python v7.5.2 historical baseline
  -> Python v7.7.0 current local production baseline
  -> ccb-legacy one-by-one Rust-compatible replacement proof
  -> optional ccbr selective intake
```

Reason: this machine has one current production `ccb` environment, now updated to `v7.7.0`; we cannot rely on a separate live `v7.5.2` runtime for acceptance. `v7.5.2` remains the historical diff baseline and prior ccbr parity anchor, not the current runtime acceptance target.

- Python historical evidence: `/home/agnitum/ccb-git` tag `v7.5.2` (`cb97581b`)
- Python current production/source evidence: `/home/agnitum/ccb-git` tag `v7.7.0`, current `HEAD=fdd11024`, `VERSION=7.7.0`
- Rust main evidence: `/home/agnitum/ccb` HEAD `80210c20`
- Owner-method status: owner-risk/work list, not confirmed-owner registry
- Python `7.6.x/7.7.0` additions are now upgrade-intake candidates against the current production baseline, not ignorable future-only items.

## Evidence snapshot: historical 7.5.2 coverage

- Python 7.5.2 daemon handlers: 26.
- Rust daemon handlers: 33.
- Python-only daemon handlers: none.
- Rust-only daemon handlers: `ask`, `cleanup`, `fault_arm`, `fault_clear`, `fault_list`, `logs`, `maintenance_tick`.
- Python 7.5.2 provider backends: 16.
- Rust provider backends: 16.
- Python-only providers: none.
- Python 7.5.2 CLI exposes `wait-any`, `wait-all`, `wait-quorum`; Rust exposes a single `wait` shape.

## Required work to claim 7.5.2 → 7.7.0 upgrade alignment

| Priority | Owner surface | Current status | Work required | Gate to close |
| --- | --- | --- | --- | --- |
| P0 | live wire/payload compatibility | Handler names are covered, but prior handoff says Python sidebar still saw `ccbd unavailable` | Capture actual Python client RPC payload/response against `ccbrd`; fix concrete field-shape mismatch only | Python sidebar connects to `ccbrd` and renders real ProjectView |
| P0 | ProjectView/sidebar projection | Prior owner matrix lists ProjectView dual owner, comms view, namespace/sidebar fields | Keep one ProjectView owner path; verify namespace, agents, comms, sidebar/window fields match Python 7.5.2 expectations | ProjectView schema tests + live sidebar smoke |
| P0 | topology/sidebar materialization | Prior matrix lists start_flow bypassing topology/materialize and missing sidebar pane creation | Ensure `ccbr start` materializes sidebar panes and applies tmux UI like Python 7.5.2 | Live start smoke shows sidebar pane(s), mouse/border metadata, no manual launch |
| P0 | inter-agent communication | Handoff says Codex coordination rules issue remains; Codex hooks must stay enabled | Prove Python-compatible ask flow A→B→A inbox through `ccbrd` without disabling hooks | Live ask smoke: A ask B, B replies, A receives inbox/reply |
| P1 | CLI wait aliases | Python 7.5.2 has `wait-any/all/quorum`; Rust uses `wait` with quorum option | Add aliases or record accepted CLI divergence | Parser/render tests for Python command forms or owner receipt for divergence |
| P1 | provider execution parity | Provider names match 16/16, but owner method requires execution gates, not just names | Verify each provider has manifest + launcher/session/readback gate or explicit unsupported mode | Provider matrix tests remain green for 16 providers |
| P1 | rolepack/current-store parity | Rolepack implementation exists, but latest 7.5.2 behavior needs receipt-level proof | Compare role install/update/current pointer behavior against Python 7.5.2 | Targeted rolepack parity tests |
| P2 | Rust-only extra ops | Rust has extra `ask`, `cleanup`, `fault_*`, `logs`, `maintenance_tick` handlers | Ensure extras do not break Python 7.5.2 clients and are documented as Rust extensions | Negative/compat tests: Python clients ignore/are unaffected |
| P2 | architecture divergence | Rust active-only polling intentionally differs from Python per-agent bridge | Preserve functional events while keeping lower CPU design; do not port Python hot polling | Completion/readback tests + CPU discipline note |

## Current 7.7.0 upgrade-intake blockers / decisions

These were not 7.5.2 parity gaps, but they must now be classified for the 7.7.0 current-production target:

- `zai` provider
- `ccb mobile` / `mobile_gateway`
- `project_sidebar_click` — closed in Rust daemon as a same-name RPC alias using existing ProjectView row resolution + focus planning
- Python 7.7.0 runtime accelerator sidecar and helper family as current Python-production behavior; route through `ccb-legacy` first, not direct `ccbr` import

## Minimal execution order

0. Diff Python `v7.5.2..v7.7.0` by owner surface and freeze which additions are required for current production compatibility.
1. Reproduce/capture Python client RPC against `ccbrd` and fix the exact ProjectView/sidebar payload mismatch.
2. Close sidebar pane materialization live smoke.
3. Close inter-agent ask/inbox live smoke with Codex hooks enabled.
4. Add Python-style `wait-any/all/quorum` only by routing to the existing Phase2 mailbox reply wait service; do not alias them to readiness `wait`. — closed in Rust CLI Slice 1.
5. Run provider and rolepack parity gates.

## Non-claims

- This does not mean direct `ccbr` parity with every Python 7.7.0 implementation detail. It means 7.7.0 production behavior must be classified and either proven through `ccb-legacy`, intentionally deferred, or explicitly marked out-of-scope.
- This does not import `ccb-legacy` or Python hot-loop architecture into `ccbr`.
- This does not disable Codex hooks.


## 2026-06-27 CodeGraph upgrade notes

- Python 7.7.0 registers `project_sidebar_click` as a daemon RPC. Rust had CLI-side sidebar click support but no same-name daemon op. Added the daemon op by reusing existing ProjectView + focus planning behavior.
- Python 7.7.0 `wait-any`, `wait-all`, and `wait-quorum` are mailbox reply waits. Rust has an existing Phase2 mailbox wait implementation, while active CLI `wait` is readiness-oriented. Slice 1 wires the Python-style commands to Phase2 mailbox wait and preserves readiness `wait`.
- Rust CodeGraph still has no `zai` or `mobile` symbols; those remain separate 7.7.0 intake surfaces.

- Slice 1 verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli test_cli_doctor_config_validate_and_pend -- --test-threads=1`; `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli test_wait_for_replies -- --test-threads=1`.

## Slice 2 ProjectView/sidebar schema receipt

- Python 7.7.0 ProjectView response contract is `{view, cache}` where `view` contains `project`, `ccbd`, `namespace`, `windows`, `agents`, and `comms`; `cache` contains `generated_at`, `ttl_ms`, and `sequence`.
- Rust daemon now has a targeted schema receipt test for those sidebar-consumed fields plus the existing sidebar row-resolution test.
- This is a schema/test receipt only; live tmux click smoke remains separate.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon project_view_response_matches_python_sidebar_shape -- --test-threads=1`; `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon sidebar_click_resolves_window_and_agent_rows -- --test-threads=1`.

## Slice 3 ask/callback continuation receipt

- Python 7.7.0 callback continuation contract preserves the original caller and parent task context when a child callback completes:
  - `to_agent = edge.callback_target_agent`
  - `from_actor = edge.original_caller`
  - `task_id = edge.original_task_id`
  - `reply_to = edge.parent_message_id`
  - `message_type = callback_continuation`
  - `route_options` include `callback_edge_id`, `callback_parent_job_id`, `callback_child_job_id`, and `callback_child_message_id`.
- Rust daemon production logic already matched this Python 7.7.0 contract; this slice adds a regression receipt to lock those fields in `callbacks_tests.rs`.
- Existing callback tests also cover plain nested ask rejection without callback/silence, silence allowance, one callback chain flow, nested callback chain waiting, child failure continuation, timeout, repair, artifact spill, cycle rejection, and depth rejection.
- This is a unit/integration receipt only; live ask/inbox smoke with real providers and Codex hooks enabled remains separate.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test callbacks_tests -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.

## Slice 4 ZAI provider intake receipt

- Python 7.7.0 adds `provider_backends/zai` as a native-cli provider:
  - manifest via `build_native_cli_manifest(provider="zai", supports_subagents=True)`
  - session file `.zai-session`
  - session attrs `zai_session_id` / `zai_session_path`
  - execution command `zai --directory <work_dir> --no-color --prompt <prompt>`
  - env state through `HOME=<zai_home>`
  - output observer reads JSONL assistant/model events, skips progress text, reports error events, and falls back to raw stdout when no JSON appears.
- Rust provider-core now includes `zai` in optional provider discovery, session filename resolution, start env (`ZAI_START_CMD`), and runtime/client spec maps.
- Rust provider-core manifest enums now include the native-cli contract values `StructuredResultStream` and `StructuredResult`, used by `zai`.
- Rust providers now register `providers::zai` in default execution/backend registries with a native-cli execution adapter, session binding, launcher, and observer tests.
- This is provider capability intake only; live `zai` CLI availability and end-to-end runtime launch remain environment-dependent smoke tests.
- Verification:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-provider-core --lib -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-provider-core --test registry_tests -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-providers --lib test_default -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-providers --test provider_zai_tests -- --test-threads=1`
  - `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`

## Slice 5 mobile gateway owner classification

- Python 7.7.0 `mobile_gateway` is not a small CLI alias. It is a separate runtime/API surface:
  - CLI service entrypoints: `prepare_mobile_gateway`, `prepare_server_mobile_gateway`, `mobile_devices_status`, `revoke_mobile_device`.
  - HTTP endpoints include `/v1/health`, `/v1/projects`, `/v1/projects/{project_id}/view`, `/v1/pairing/claim`, `/v1/devices/me`, `/v1/devices/{device_id}/revoke`, lifecycle/focus endpoints, and terminal endpoints.
  - State owners include `MobileGatewayPairingStore`, `MobileGatewayProjectRegistry`, host state directory, pairing payloads, device revocation, and local relay harness.
  - It calls into the existing `ccbd` ProjectView/focus/lifecycle/terminal operations rather than owning those facts itself.
- Rust `ccbr` currently has no equivalent `mobile` CLI command, gateway service, pairing store, project registry, or terminal relay module.
- Owner classification:
  - Command owner: CLI `mobile` command surface.
  - Runtime/API owner: mobile gateway HTTP server.
  - Readback owner: daemon ProjectView/lifecycle/focus/terminal APIs, not mobile gateway.
  - Credential/device owner: mobile pairing store and device revocation records.
  - Relay owner: local relay harness / outbound relay client.
- Minimum safe Rust intake should be split into separate commits:
  1. parser/model receipt for `ccbr mobile serve|devices|revoke` command shapes;
  2. pure state module for pairing/device store with Python fixture tests;
  3. read-only gateway service for health/projects/view against existing daemon client;
  4. focus/lifecycle/terminal mutation endpoints after read-only contract is green;
  5. relay harness last.
- Non-claim: this slice does not implement mobile gateway. It prevents a fake partial implementation by naming the real owner surfaces first.

## Slice 6 mobile parser/model receipt

- Rust CLI now recognizes `mobile` as a first-class command instead of treating it as a start-agent token.
- Parsed command coverage matches the Python 7.7.0 parser contract for:
  - `mobile serve [--listen <addr>] [--public-url <url>] [--route-provider lan|tailnet|cloudflare_tunnel|relay]`
  - `mobile devices`
  - `mobile revoke <device_id>`
- `crate::models::ParsedCommand` also has a `Mobile(ParsedMobileCommand)` variant for Python dataclass parity.
- Runtime service remains intentionally unimplemented and returns `Command not yet implemented: mobile gateway`; gateway state/API work remains in later slices.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli test_cli_mobile_parser_receipts -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.

## Slice 7 mobile pairing/device store receipt

- Rust daemon now has a pure `mobile_gateway::pairing` state owner that mirrors Python 7.7.0 mobile pairing storage names and public shapes:
  - `gateway.json`
  - `pairing-tokens.jsonl`
  - `devices.json`
  - `audit.jsonl`
- Covered operations match the first Python mobile state slice:
  - write gateway state;
  - create pairing payload with `pairing_code`, `claim_endpoint`, scopes, and expiry;
  - claim a pairing into a stored device with hashed bearer token;
  - authenticate a bearer token and update `last_seen_at`;
  - list public device payloads;
  - host-side revoke with Python-compatible `status`, `device`, and `revoked_terminal_count` fields.
- Error receipts preserve Python reason/status pairs for missing/invalid pairing codes, already claimed pairings, duplicate devices, missing device IDs, invalid tokens, and missing devices.
- This remains a state module only; HTTP routes, project registry, terminal handles, and relay behavior remain later slices.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_pairing_tests -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`; `git diff --check`.

## Slice 8 mobile read-only gateway service receipt

- Rust daemon now has a read-only `mobile_gateway::service` contract for the Python 7.7.0 mobile gateway owner surface without starting an HTTP server:
  - `health_payload()` mirrors `/v1/health` ok/degraded shape and ccbd health summary fields;
  - `projects_payload()` mirrors `/v1/projects` registry projection including unreachable project fallback;
  - `project_view_payload(project_id)` calls a project client and redacts private namespace fields (`socket_path`, `session_name`) like Python.
- Service capabilities preserve Python capability split: base `http_json`/`project_view`, plus pairing/device/lifecycle/focus/terminal/file capabilities only when a pairing store is configured.
- This is still service-contract only; HTTP server binding, bearer auth dispatch, mutation routes, terminal routes, and relay remain later slices.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_service_tests -- --test-threads=1`; `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_pairing_tests -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`; `git diff --check`.

## Slice 9 mobile dispatch/auth receipt

- Rust mobile gateway service now has route-level dispatch contract for the first Python 7.7.0 authenticated surfaces, still without binding a real HTTP server:
  - `GET /v1/health` and `GET /v1/projects`;
  - authenticated `GET /v1/projects/{project_id}/view`;
  - authenticated `GET /v1/devices/me`;
  - unauthenticated `POST /v1/pairing/claim`;
  - bearer-authenticated self `POST /v1/devices/{device_id}/revoke`.
- Pairing store now supports Python-compatible device self-revoke (`self_revoked`) and rejects cross-device revoke with status 403 / message `device can only revoke itself in G2`.
- This still does not bind an HTTP socket and does not implement lifecycle/focus/terminal/message/file/relay mutation routes.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_service_tests -- --test-threads=1`; `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_pairing_tests -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`; `git diff --check`.

## Slice 10 mobile focus/lifecycle mutation receipt

- Rust mobile gateway service now has Python 7.7.0 route contract coverage for daemon-backed mutation dispatch without binding a real HTTP server:
  - bearer-authenticated `POST /v1/projects/{project_id}/focus-agent`;
  - bearer-authenticated `POST /v1/projects/{project_id}/focus-window`;
  - bearer-authenticated `POST /v1/projects/{project_id}/lifecycle` for `wake`, `open`, `close`, and `stop`.
- Focus routes call the project client focus owner, then return a redacted ProjectView plus `focus` payload like Python.
- Lifecycle routes preserve Python effects: `wake -> already_running`, `open -> opened`, `close -> mobile_view_closed`, `stop -> ccbd_stop_requested`; all keep `tmux_kill_server=false`.
- Scope boundaries remain explicit: this slice does not implement terminal opening, message/file routes, relay, or actual HTTP socket binding.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_service_tests -- --test-threads=1`; `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_pairing_tests -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`; `git diff --check`.
