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

## Slice 11 mobile terminal owner classification

- Python 7.7.0 mobile terminal is a separate owner surface, not a small extension of focus/lifecycle routes.
- Terminal owner surfaces found by CodeGraph:
  - terminal handle state owner: `MobileGatewayPairingStore.create_terminal_handle`, `authenticate_terminal_token`, `record_terminal_input_sequence`, terminal close/disconnect/revoke state in `terminal-tokens.jsonl`;
  - terminal target validation owner: ProjectView namespace epoch, target agent/window/pane summary, geometry, and stale epoch rejection;
  - terminal readback owner: `terminal_history_payload` with `TerminalHistoryTarget`, tmux scrollback source, generated/stale/block fields;
  - websocket stream owner: open frame validation, resume cursor, output pump, input/paste/resize frames, close/disconnect semantics;
  - transport owner: HTTP upgrade/websocket framing and safe close/error frames.
- Safe Rust intake order for terminal parity:
  1. add terminal token state methods to the pairing store with Python fixture tests;
  2. add pure target validation and terminal-history payload contract tests against fake ProjectView/history provider;
  3. add `POST /v1/projects/{project_id}/terminals` dispatch contract returning terminal handle + websocket URL;
  4. only then add websocket transport/session adapter, because it owns live tmux I/O and replay/resume semantics.
- Non-claim: focus/lifecycle parity does not imply terminal parity. Terminal must remain blocked until the above owner gates are green.

## Slice 12 mobile terminal token state receipt

- Rust pairing store now covers the Python 7.7.0 terminal token state owner without websocket/tmux transport:
  - `create_terminal_handle` appends `terminal-tokens.jsonl` and returns terminal token, expiry, epoch, and target summary;
  - `authenticate_terminal_token` validates token, closed/revoked/expired/device-revoked state, and resume cursor rules;
  - `record_terminal_input_sequence` rejects replayed input sequences;
  - `record_terminal_output_sequence` advances output cursor monotonically;
  - `mark_terminal_disconnected` records disconnect reason/time;
  - `close_terminal_handle` records close reason/time.
- Tests lock Python-compatible reason/status boundaries for replay, missing/stale resume cursor, closed terminal, invalid token, and device-revoked terminal access.
- This still does not implement target validation, terminal route payloads, websocket framing, tmux session I/O, terminal history, or relay.
- Verification: `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_pairing_tests -- --test-threads=1`; `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test mobile_gateway_service_tests -- --test-threads=1`; `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`; `git diff --check`.

## 2026-06-27 non-mobile parity checkpoint

Mobile intake is paused by user direction. Current focus returns to the remaining non-mobile alignment gates.

Static / targeted verification completed in `/home/agnitum/ccb`:

- ProjectView/sidebar schema and click receipts:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon project_view_response_matches_python_sidebar_shape -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon sidebar_click_resolves_window_and_agent_rows -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli --test sidebar_click_tests -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli --test sidebar_resize_sync_tests -- --test-threads=1`
- Python-style mailbox wait receipts:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli --test cli_wait_tests -- --test-threads=1`
- Ask/callback continuation receipts:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test callbacks_tests -- --test-threads=1`
- Rolepack/current-store receipts:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-agents --test rolepack_tests -- --test-threads=1`
- Provider execution / launcher / readback receipts:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-providers --tests -- --test-threads=1`
- Start/sidebar materialization-adjacent receipts:
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test start_flow_launch_context_tests -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test start_runtime_layout_tests -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-daemon --test start_preparation_tests -- --test-threads=1`
  - `cargo test --manifest-path rust/Cargo.toml -p ccbr-cli --test smoke_test -- --test-threads=1`

All commands above completed successfully.

Remaining non-mobile gates are live/evidence gates, not obvious code gaps from the targeted suite:

1. live Python/sidebar client RPC against `ccbrd` with real ProjectView rendering;
2. live `ccbr start` sidebar pane materialization smoke in tmux;
3. live A→B→A ask/inbox smoke with Codex hooks enabled;
4. ccb-legacy performance acceptance evidence: active-vs-idle split, bridge CPU source split, before/after Slice A/B CPU proof, and residue cleanup proof.

## 2026-06-27 non-mobile live smoke continuation

Mobile remains paused. Live smoke used isolated root `.trellis/workspace/luck/live-smoke/ccbr-nonmobile-codex`; Codex hooks were not disabled.

Findings fixed:
- `ccbr project-view` now dispatches through an explicit `commands::project_view()` path instead of borrowing `status()` dispatch. The command still renders the daemon ProjectView in the existing human-readable style, but the RPC contract is now covered by a targeted test.
- `ccbr start` with no explicit agents no longer falls back to a literal `default` agent when no project config exists. It now uses built-in default agents: `agent1`, `agent2`, `agent3`, `ccbr_self`.

Live evidence:
- `ccbr --project <smoke-root> ping ccbrd` returned `pong`.
- `ccbr --project <smoke-root> start` returned `Started agents: agent1, agent2, agent3, ccbr_self`.
- Project private tmux socket showed four panes: `%0/%1/%2/%3` with CCBR pane titles.
- `ccbr ask agent2 --from agent1 <message>` returned an accepted job and `inbox agent2 --detail` showed the pending item.

Remaining non-mobile gaps:
- `inbox agent2` rendered `pending=0` while listing a pending job; mailbox count rendering needs follow-up.
- `queue agent2 --detail` rendered `(no agents)` immediately after accepted submit; queue rendering/target filtering needs follow-up.
- Provider execution parity still needs real provider hook-enabled smoke, not just shell-pane/default-agent materialization.

## 2026-06-27 mailbox render follow-up

Follow-up fixed the remaining shell-pane ask observer output found in the live smoke:
- CLI inbox renderer now prefers canonical `item_count`/items over stale summary `pending_reply_count` when rendering daemon mailbox payloads.
- CLI queue renderer now accepts canonical single-agent `agent` payloads in addition to legacy `agents[]` payloads.

Live evidence after fix, again in isolated smoke root with hooks not disabled:
- `ccbr ask agent2 --from agent1 <message>` returned an accepted job.
- `ccbr inbox agent2 --detail` rendered `Inbox for agent2 (pending=1)` and showed the pending job.
- `ccbr queue agent2 --detail` rendered `agent2: depth=1 active=-` instead of `(no agents)`.
- `scripts/ccbr-test-cleanup.sh` reclaimed the isolated ccbr smoke runtime and did not touch ccb production.

Remaining non-mobile gaps after this slice:
- Real provider hook-enabled completion smoke is still pending; current live proof covers daemon/start/tmux shell panes plus mailbox observer rendering.
- ccb-legacy performance acceptance evidence remains a separate bloodline gate.

## 2026-06-27 provider-config launch smoke

Provider execution parity gate was advanced with an isolated real provider configuration, still with mobile paused and without disabling Codex hooks:

```toml
version = 2
default_agents = ["codex"]

[agents.codex]
provider = "codex"
target = "codex"

[windows]
main = "codex:codex"
```

Evidence:
- `ccbr config validate` loaded `.ccbr/ccbr.config` as `project_config` with one default `codex` agent.
- `ccbr start codex` returned `Started agents: codex`.
- `ccbr project-view` rendered `codex [idle] codex (%1)`, proving provider identity reached ProjectView.
- Private tmux pane showed a `node` command for the codex provider process.
- Test smoke roots and private tmux/socket/process residues were reclaimed after the run; production ccb under `/home/agnitum/o13` was not touched.

Boundary:
- This proves provider-config launch/materialization, not full provider completion/reply loop. Full hook-enabled completion remains the next parity gate.

## 2026-06-27 provider completion smoke attempt and cleanup repair

Attempted a hook-enabled single Codex provider completion smoke in an isolated root:
- `ccbr start codex` succeeded.
- `ccbr ask codex --from ccbr_self "Reply exactly: <token>"` returned an accepted job.
- `ccbr wait codex --timeout 60` reported readiness, not job completion.
- `ccbr inbox codex --detail` still showed the job pending with no reply.

Conclusion:
- Provider launch/materialization parity is proven for the configured Codex provider path.
- Provider completion/reply parity is not proven yet; the next gate must wait/trace a concrete job id or observe terminal reply delivery.

Cleanup finding and fix:
- The completion smoke exposed that test tmux sockets can be created under `/run/user/0/ccbr-runtime`, and a live debug `ccbrd` can recreate tmux after the cleanup script's first tmux sweep.
- `scripts/ccbr-test-cleanup.sh` now kills leaked debug `ccbrd` processes before a second tmux sweep, then removes runtime sockets.
- Verification: `bash -n scripts/ccbr-test-cleanup.sh`; `bash scripts/ccbr-test-cleanup.sh`; `/run/user/0/ccbr-runtime` had no remaining tmux sockets afterward.

## 2026-06-27 hook-enabled provider completion proof

The earlier provider completion attempt used an isolated root without `.codex/hooks`, so Codex blocked `UserPromptSubmit`. That evidence was rejected for completion acceptance because hooks must remain enabled and functional.

A corrected smoke copied the repository `.codex` hook files into the isolated root before launch. Evidence retained under `evidence/ccbr-provider-completion-hooks/`:

- `ccbr ask codex --from ccbr_self "Reply exactly: <token>"` returned accepted job `job_d689f3f98d77`.
- `ccbr trace job_d689f3f98d77` rendered `job_d689f3f98d77 [codex] completed`.
- `ccbr inbox codex --detail` rendered `pending=0`.
- Captured Codex pane showed `UserPromptSubmit hook (completed)` and the exact provider reply token `CCBR_PROVIDER_HOOK_SMOKE_1782545229`.
- Smoke runtime was reclaimed with `scripts/ccbr-test-cleanup.sh`; no production ccb runtime was touched.

Provider execution parity status for this slice: passed for single Codex provider submit -> hook -> reply -> trace terminal path. Multi-agent callback/order scenarios remain separate gates.

## 2026-06-27 Provider scope correction after owner decision

User decision for non-mobile 7.7.0 intake:

- P1 provider live acceptance is limited to `codex`, `kimi`, and `claude`.
- Other providers are not current production acceptance blockers because they are not used locally.
- `zai` is explicitly sealed because the upstream source admitted an unofficial/shanzhai provider by mistake; Rust must not advertise it by default.
- Claude token is restored, so Claude can re-enter live acceptance after Codex/Kimi.

Implementation consequence:

- Keep existing ZAI source/tests as archived code only; do not delete them in this slice.
- Remove ZAI from default optional provider discovery, default runtime/client spec maps, and default provider execution/backend registries.
- Do not run ZAI live tests or treat ZAI as a 7.7.0 parity blocker unless a later owner decision explicitly unseals it.

P2 decision:

- Python 7.7.0 helper/runtime-accelerator family is classified as covered by ccbr native Rust daemon architecture unless a helper exposes a user-visible contract not already covered.
- Do not import Python `.ccb` runtime accelerator sidecar into ccbr. Share only narrow parser/observer logic later if a measured gap appears.
