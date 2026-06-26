# Owner Alignment v1.1 — Python CCB 7.5.2 vs Rust CCBR

Date: 2026-06-26
Owner spec: `/mnt/g/owner/software_owner_governance_spec_v1_1.md`

## Method learned from v1.1

Rules applied here:

- Ownership means accountability for business truth or system responsibility; execution is not ownership.
- Do not infer ownership from git, path, UI/runtime state, or the fact that code exists.
- Every record has exactly one MECE surface: `domain_truth`, `policy`, `interface`, `capability`, `projection`, `evidence_admission`, or `lifecycle_gate`.
- Interfaces must name both provider and consumer owners; cross-system interface owners must differ.
- Projections must name `source_truth_ref` and use `local_projection` mode.
- Capability owners maintain behavior but do not own truth.

## System owner dictionary

| Owner id | Meaning | Non-claim |
|---|---|---|
| `python_ccb_7_5_2_reference_contract_owner` | Reference owner for client-facing ccbd socket semantics and user-visible behavior. | Does not own Rust implementation internals or performance architecture. |
| `rust_ccbrd_runtime_owner` | Current owner for ccbrd socket provider, daemon lifecycle, registry, namespace, and mailbox runtime. | Cannot silently diverge from Python client-facing interface contracts. |
| `rust_ccbr_provider_runtime_owner` | Current owner for provider session payloads, prompt dispatch, structured transcript polling, and active-only heartbeat behavior. | Must not disable or mask Codex hooks; does not own Python low-performance bridge design. |
| `rust_sidebar_consumer_owner` | Current owner for `tools/ccb-agent-sidebar` rendering and click actions. | Projection consumer only; cannot redefine daemon truth. |
| `trellis_codegraph_evidence_owner` | Evidence/planning/indexing helper. | Not owner truth. |
| `ccb_legacy_rust_mirror_owner` | Separate Rust reverse-mirror branch for upstream original merge convenience. | Never merges with ccbr mainline; sync only equivalent Rust owner fixes. |

## Registered interface coverage

Extraction from Python `lib/ccbd/app_runtime/handlers.py` and Rust `rust/crates/ccbr-daemon/src/handlers/mod.rs`:

- Python registered ops: 26.
- Rust registered ops: 33.
- Python-only ops: none.
- Rust-only local extensions: `ask`, `cleanup`, `fault_arm`, `fault_clear`, `fault_list`, `logs`, `maintenance_tick`.

Conclusion: interface registration coverage is not the current blocker. Remaining risk is owner behavior parity and runtime readback.

## Canonical owner records

| field | surface | accountable_owner | ownership_mode | status | source_anchor | interface_provider_owner | interface_consumer_owner | interface_boundary | delegation_target | source_truth_ref | shared_kernel | Python 7.5.2 anchor | Rust CCBR anchor | Comparison / gap |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `ccbd_rpc_registry` | interface | `rust_ccbrd_runtime_owner` | primary | confirmed_owner | `lib/ccbd/app_runtime/handlers.py`; `handlers/mod.rs` | `rust_ccbrd_runtime_owner` | `python_ccb_7_5_2_reference_contract_owner` | cross_system | null | null | true | `register_handler(...)` 26 ops | `reg.register(...)` 33 ops | Rust covers every Python op; local extensions are not Python truth. |
| `submit/ask_message_submission` | interface | `rust_ccbrd_runtime_owner` | delegated | candidate | `handlers/submit.py`; `handlers/submit.rs`; `ccbr-cli/src/commands.rs` | `rust_ccbrd_runtime_owner` | `python_ccb_7_5_2_reference_contract_owner` | cross_system | `rust_ccbr_provider_runtime_owner` | null | true | `dispatcher.submit(envelope).to_record()` | `handle_submit` + CLI calls daemon `submit` | Interface owner is ccbrd; delivery delegated to provider runtime. Must keep Python `submit` semantics, not Rust-only `ask`, for interop. |
| `rust_local_ask` | capability | `rust_ccbrd_runtime_owner` | primary | confirmed_owner | `handlers/ask.rs` | null | null | internal | null | null | false | none | Rust local `ask` handler | Local convenience only; cannot replace Python `submit` owner. |
| `project_view_payload` | projection | `rust_ccbrd_runtime_owner` | local_projection | confirmed_owner | `project_view/service.py`; `handlers/project_view.rs`; `tools/ccb-agent-sidebar/src/model.rs` | `rust_ccbrd_runtime_owner` | `rust_sidebar_consumer_owner` | internal | null | `python_ccb_7_5_2_reference_contract_owner:project_view_shape` | true | Python `ProjectView` service emits `{view, cache}` | Rust handler emits sidebar shape; Rust sidebar parses it | Projection owner is Rust local readback; source truth is Python-compatible shape. |
| `sidebar_red_x_shutdown` | lifecycle_gate | `rust_ccbrd_runtime_owner` | primary | confirmed_owner | user confirmation; `workspace-shutdown.md`; `handlers/shutdown.rs`; `app.rs` | null | null | internal | null | null | false | Python graceful shutdown/stop flow | Rust `shutdown -> stop_all(force=true)` local rule | Intentional local divergence: red X means full workspace exit in this project. |
| `mailbox_inbox_ack` | interface | `rust_ccbrd_runtime_owner` | primary | confirmed_owner | `handlers/inbox.py`; `handlers/ack.py`; Rust `inbox.rs`; `ack.rs`; `ccbr-mailbox` | `rust_ccbrd_runtime_owner` | `python_ccb_7_5_2_reference_contract_owner` | cross_system | null | null | true | `dispatcher.inbox`, `dispatcher.ack_reply` | mailbox-control readback and `inbound_event_id` support | Interface is aligned; ongoing risk is upstream provider completion delivering a reply into mailbox. |
| `dispatcher_trace_queue_retry_resubmit` | interface | `rust_ccbrd_runtime_owner` | primary | confirmed_owner | Python dispatcher handlers; Rust dispatcher/mailbox handlers | `rust_ccbrd_runtime_owner` | `python_ccb_7_5_2_reference_contract_owner` | cross_system | null | null | true | Python dispatcher/message-bureau handlers | Rust rejects invalid trace targets and uses mailbox lineage | Behavior now follows Python message-bureau owner; legacy agent-name trace is rejected by design. |
| `project_restart_reload_clear_focus` | lifecycle_gate | `rust_ccbrd_runtime_owner` | primary | confirmed_owner | Python project handlers; Rust project handlers | null | null | internal | null | null | true | Python in-place/reload/focus/clear services | Rust handlers execute topology recreation/reload/focus/clear | Rust may use DDD/recreate internals when response contract remains compatible and divergence is recorded. |
| `provider_codex_session_polling` | capability | `rust_ccbr_provider_runtime_owner` | primary | confirmed_owner | Python Codex provider reference; Rust Codex provider; `codex-wire-protocol.md` | null | null | internal | null | null | true | Python bridge/per-agent tight polling | Rust active-only provider execution + structured JSONL | Intentional performance divergence: do not copy Python per-agent bridge/tight polling; never disable hooks. |
| `provider_claude_session_polling` | capability | `rust_ccbr_provider_runtime_owner` | primary | candidate | Python Claude provider reference; Rust provider launcher/session/reader | null | null | internal | null | null | true | Python Claude projects root/session discovery under managed home | Rust provider reader requires managed session binding plus Claude project log ingestion | P0 gap found by live smoke: prompt reached Claude and Claude replied in managed JSONL, but daemon did not terminalize the job or deliver inbox. `claude_projects_root` is now added as payload parity, but the post-fix live smoke still showed readback incomplete; root cause remains in Claude polling/reader-state ingestion, not RPC registration. |
| `codex_hook_policy` | policy | `rust_ccbr_provider_runtime_owner` | primary | confirmed_owner | user hard rule; `codex-wire-protocol.md`; launch args | null | null | internal | null | null | false | Python keeps rendered CCB rules | Rust uses developer instructions/session binding; Codex hooks remain enabled | Hook disabling/masking is rejected. Coordination must be solved by launch args, session payloads, and polling. |
| `test_resource_cleanup` | lifecycle_gate | `rust_ccbrd_runtime_owner` | primary | candidate | `scripts/ccbr-test-cleanup.sh`; live smoke cleanup evidence | null | null | internal | null | null | false | none | Rust test cleanup only | P0 operational guardrail: cleanup must reclaim ccbr-runtime tmux orphans without touching Python `.ccb`/`ccb` state. |
| `ccb_legacy_sync_rule` | policy | `ccb_legacy_rust_mirror_owner` | primary | candidate | branch `ccb-legacy`; user instruction | null | null | internal | null | null | false | Python original import path | Separate Rust mirror branch | Sync equivalent Rust fixes; do not force non-equivalent tests or pollute Python `ccb`. |

## Priority findings

### P0 — Provider completion readback is the current live interop blocker

Live smoke in `/mnt/d/dapro-ass` with real agents proved:

- `ccbrd` started and `project-view` returned real `agent1/agent2/agent3` state.
- `ccbr_test ask agent3 from agent1 -- "Reply exactly: TOKEN"` returned `accepted`.
- Claude JSONL under the managed home recorded the user request and exact assistant token.
- `ccbr inbox --detail agent1` stayed empty and job remained `running`.

Owner diagnosis:

- Interface owner (`submit`) worked.
- Capability owner (Claude provider runtime) dispatched the prompt.
- Projection/readback owner (mailbox inbox) had no reply because provider completion polling did not ingest the managed Claude project log.
- Root cause owner field: `provider_claude_session_polling`.
- The first payload gap was real: Rust session payload contained `completion_artifact_dir` and `tmux_socket_path` but not managed `claude_projects_root`, so `ClaudeLogReader` could fall back to the wrong default home.
- Post-fix smoke proved that payload parity alone is not sufficient yet: the session payload included `claude_projects_root`, Claude JSONL still contained the exact reply, but queue/trace remained `running` and inbox stayed empty.

Minimal correction path:

1. Keep `claude_projects_root = <runtime_dir>/home/.claude/projects` in Rust simple Claude session payload as required parity.
2. Inspect the active execution `reader_state`, heartbeat promotion, and `ClaudeLogReader` offset/session reset path to find why the managed JSONL reply is not ingested.
3. Keep hooks enabled.
4. Validate with provider launcher unit test and live ask smoke.
5. Sync to `ccb-legacy` only when the same Rust owner surface exists there.

### P1 — Owner docs need v1.1 terminology, not older mixed surface names

Existing `wire-protocol-gap.md` is useful evidence, but its `surface` vocabulary includes older mixed labels such as `runtime integration`. Under v1.1 those must be represented as one of the seven MECE surfaces, usually `capability` or `lifecycle_gate`, with detail in comparison notes.

### P2 — Rust-only extensions must stay explicitly non-Python

`ask`, `cleanup`, `fault_*`, `logs`, and `maintenance_tick` are Rust-local capabilities. They are allowed only as extensions and cannot become evidence that Python interop is complete.

## Verification in this pass

- Re-extracted registered ops directly from `/home/agnitum/ccb-v7.5.2/lib/ccbd/app_runtime/handlers.py` and `rust/crates/ccbr-daemon/src/handlers/mod.rs` with a multiline `register_handler` / `reg.register` scan.
- Result remains: Python 26 ops; Rust 33 ops; Python-only none; Rust-only `ask`, `cleanup`, `fault_arm`, `fault_clear`, `fault_list`, `logs`, `maintenance_tick`.
- Removed a stale Rust registry TODO that contradicted the current tested `submit` owner behavior.
- Resource cleanup evidence after smoke interruption: `CCB_TEST_ROOTS=/mnt/d/dapro-ass bash scripts/ccbr-test-cleanup.sh` left no targeted ccbrd/tmux/provider process, removed ccbr daemon state, and left only `.ccbr/ccbr.config` plus `.ccbr/bin` in the test project.

## Non-claims

- Trellis, CodeGraph, and this document are evidence/admission helpers, not owner truth.
- Python 7.5.2 is the reference contract owner, not the owner of Rust internals.
- Rust provider optimizations are allowed only when the Python-facing contract remains compatible.
- UI/sidebar state is projection/readback only.
- Codex hooks must remain enabled; hook suppression is not an owner-alignment strategy.
- `ccb-legacy` is not Python `ccb` and is not merged with ccbr mainline.
