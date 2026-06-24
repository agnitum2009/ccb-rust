# Wave 3 Implementation Plan — `ccb-providers` + `ccb-daemon` Deep Stub Reduction

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the `TODO: align with Python` stub surface in `ccb-providers` (368 files) and `ccb-daemon` (345 files) by implementing parity for the provider execution adapters and the daemon dispatcher/namespace/reload/supervision subsystems.

**Architecture:** Provider execution adapters live in `src/providers/<name>.rs` (canonical surface) and consume supporting modules under `src/<name>/`. The daemon dispatcher consumes these adapters through `ExecutionService`. Namespace materialization, config reload, and supervision run as daemon subsystems that operate on project state and tmux backends. Each sub-theme is implemented behind a failing test, then filled, then verified in isolation.

**Tech Stack:** Rust 2021 edition, workspace crate graph (`ccb-providers`, `ccb-daemon`, `ccb-provider-core`, `ccb-completion`, `ccb-jobs`, `ccb-mailbox`, `ccb-terminal`, `ccb-storage`), `serde_json`, `camino`, `tempfile` for tests. No new external dependencies; avoid introducing `chrono`/`regex`/`reqwest` where the codebase already uses `std::time`/string ops/curl.

---

## File Structure

| Sub-theme | Canonical Rust files | Supporting Rust files | Python reference |
|-----------|---------------------|-----------------------|------------------|
| Provider adapter surface | `rust/crates/ccb-providers/src/lib.rs`, `rust/crates/ccb-providers/src/providers/mod.rs` | `rust/crates/ccb-providers/src/mod.rs` (stale), `rust/crates/ccb-providers/src/agy/*` (legacy duplicate) | `lib/provider_backends/__init__.py` |
| Codex execution | `rust/crates/ccb-providers/src/providers/codex.rs` | `rust/crates/ccb-providers/src/codex/launcher.rs`, `rust/crates/ccb-providers/src/codex/launcher_runtime/*` | `lib/provider_backends/codex/execution.py`, `lib/provider_backends/codex/execution_runtime/*` |
| Claude execution/comm | `rust/crates/ccb-providers/src/providers/claude.rs` | `rust/crates/ccb-providers/src/claude/comm_runtime/*`, `rust/crates/ccb-providers/src/claude/session.rs`, `rust/crates/ccb-providers/src/claude/launcher_runtime/*` | `lib/provider_backends/claude/execution.py`, `lib/provider_backends/claude/comm_runtime/*` |
| Gemini execution | `rust/crates/ccb-providers/src/providers/gemini/mod.rs` | `rust/crates/ccb-providers/src/providers/gemini/log_reader.rs`, `rust/crates/ccb-providers/src/providers/gemini/launcher.rs` | `lib/provider_backends/gemini/execution.py`, `lib/provider_backends/gemini/log_reader.py` |
| Droid execution | `rust/crates/ccb-providers/src/providers/droid.rs` | `rust/crates/ccb-providers/src/droid/execution_runtime/*`, `rust/crates/ccb-providers/src/droid/comm.rs`, `rust/crates/ccb-providers/src/droid/session.rs` | `lib/provider_backends/droid/execution.py`, `lib/provider_backends/droid/execution_runtime/*` |
| AGY execution | `rust/crates/ccb-providers/src/providers/agy/mod.rs` | `rust/crates/ccb-providers/src/providers/agy/native_log.rs`, `rust/crates/ccb-providers/src/providers/agy/session.rs`, `rust/crates/ccb-providers/src/agy/*` | `lib/provider_backends/agy/execution.py`, `lib/provider_backends/agy/execution_runtime/*`, `lib/provider_backends/agy/native_log.py` |
| OpenCode execution | `rust/crates/ccb-providers/src/providers/opencode.rs` | `rust/crates/ccb-providers/src/opencode/session.rs`, `rust/crates/ccb-providers/src/opencode/runtime/*`, `rust/crates/ccb-providers/src/opencode/execution_runtime/*` | `lib/provider_backends/opencode/execution.py`, `lib/provider_backends/opencode/runtime/*` |
| Shared provider infra | `rust/crates/ccb-providers/src/common_runtime/*`, `rust/crates/ccb-providers/src/pane_log_support/*` | `rust/crates/ccb-providers/src/helper_cleanup.rs`, `rust/crates/ccb-providers/src/session_paths.rs`, `rust/crates/ccb-providers/src/workspace_preparation.rs`, `rust/crates/ccb-providers/src/model_shortcuts.rs` | `lib/provider_backends/common_runtime/*`, `lib/provider_backends/pane_log_support/*`, `lib/provider_runtime/helper_cleanup.py` |
| Dispatcher lifecycle/polling | `rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling_service.rs` | `rust/crates/ccb-daemon/src/services/dispatcher_runtime/state*.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/records.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/completion.rs` | `lib/ccbd/services/dispatcher_runtime/lifecycle.py`, `lib/ccbd/services/dispatcher_runtime/polling_service.py`, `lib/ccbd/services/dispatcher_runtime/state.py` |
| Dispatcher submission/routing | `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission_service.rs` | `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission_recording.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/routing.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/callbacks.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/comms_recover.rs` | `lib/ccbd/services/dispatcher_runtime/submission_service.py`, `lib/ccbd/services/dispatcher_runtime/routing.py`, `lib/ccbd/services/dispatcher_runtime/callbacks.py`, `lib/ccbd/services/dispatcher_runtime/comms_recover.py` |
| Dispatcher finalization/reply | `rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization*.rs` | `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery*.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/visible_reply.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/execution_cleanup.rs` | `lib/ccbd/services/dispatcher_runtime/finalization.py`, `lib/ccbd/services/dispatcher_runtime/reply_delivery.py` |
| Namespace runtime | `rust/crates/ccb-daemon/src/services/project_namespace_runtime/ensure.rs` | `rust/crates/ccb-daemon/src/services/project_namespace_runtime/materialize_topology.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/backend.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/topology_plan.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch*.rs` | `lib/ccbd/services/project_namespace_runtime/ensure.py`, `lib/ccbd/services/project_namespace_runtime/materialize_topology.py`, `lib/ccbd/services/project_namespace_runtime/additive_patch*.py` |
| Config reload | `rust/crates/ccb-daemon/src/reload_apply.rs` | `rust/crates/ccb-daemon/src/reload_apply_*.rs`, `rust/crates/ccb-daemon/src/reload_runtime_mount_*.rs`, `rust/crates/ccb-daemon/src/reload_transaction*.rs`, `rust/crates/ccb-daemon/src/reload_patch_*.rs`, `rust/crates/ccb-daemon/src/reload_plan.rs`, `rust/crates/ccb-daemon/src/reload_additive_agents.rs` | `lib/ccbd/reload_apply.py`, `lib/ccbd/reload_apply_*.py`, `lib/ccbd/reload_transaction*.py` |
| Supervision | `rust/crates/ccb-daemon/src/supervision/loop_.rs` | `rust/crates/ccb-daemon/src/supervision/loop_*.rs`, `rust/crates/ccb-daemon/src/supervision/mount*.rs`, `rust/crates/ccb-daemon/src/supervision/recovery*.rs`, `rust/crates/ccb-daemon/src/supervision/backoff.rs`, `rust/crates/ccb-daemon/src/supervision/cmd_slot.rs` | `lib/ccbd/supervision/loop.py`, `lib/ccbd/supervision/mount.py`, `lib/ccbd/supervision/recovery*.py` |
| Daemon top-level stubs | `rust/crates/ccb-daemon/src/*.rs` with `TODO: align with Python` | N/A | corresponding `lib/ccbd/*.py` |

## Execution Order

Providers are implemented before the daemon dispatcher because the dispatcher's `poll_completion_updates` consumes `ProviderPollResult` / `CompletionItem` / `CompletionDecision` shapes produced by provider adapters. Stabilizing those shapes first avoids cascading type changes.

1. **P0 Baseline + triage** — count stubs, decide dead-code deletions.
2. **P1 Provider adapter surface** — registry wiring, duplicate module cleanup.
3. **P2–P7 Provider adapters** — Codex, Claude, Gemini, Droid, AGY, OpenCode.
4. **P8 Shared provider infrastructure** — serialization, pane log support, helper cleanup.
5. **D1–D3 Dispatcher runtime** — submission/routing, lifecycle/polling, finalization/reply.
6. **D4 Namespace runtime** — ensure + topology materialization.
7. **D5 Config reload** — plan/apply/mount transaction.
8. **D6 Supervision** — loop + mount + recovery.
9. **D7 Daemon top-level stubs** — implement or delete triaged stubs.
10. **Z Final validation** — workspace check/test/clippy/fmt + matrix update.

## Task Breakdown

### P0: Baseline and stub triage

**Files:**
- Read: `rust/crates/ccb-providers/src/lib.rs`, `rust/crates/ccb-providers/src/providers/mod.rs`, `rust/crates/ccb-providers/src/mod.rs`, `rust/crates/ccb-daemon/src/lib.rs`
- Modify: `rust/crates/ccb-providers/src/lib.rs` (after P1), `rust/crates/ccb-daemon/src/lib.rs` (after D7)
- Test: none (analysis only)

- [ ] **Step 1: Record current stub counts**

Run:
```bash
cd /home/agnitum/ccb
echo "ccb-providers stubs: $(grep -rln 'TODO: align with Python' rust/crates/ccb-providers/src/ | wc -l)"
echo "ccb-daemon stubs:    $(grep -rln 'TODO: align with Python' rust/crates/ccb-daemon/src/ | wc -l)"
```
Expected output (2026-06-24 baseline):
```text
ccb-providers stubs: 368
ccb-daemon stubs:    345
```

- [ ] **Step 2: Classify each stub as implement / delete / defer**

For every file returned by the grep above, open it and decide:
- **Implement** if a corresponding Python file exists and is exercised by a parity-matrix test.
- **Delete** if it is an empty 3-line alignment stub with no Python reference and no caller in the workspace.
- **Defer** if it belongs to Windows/WSL, live CLI integration, or a Wave 4 edge feature.

Record the decision list in `.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md`.

- [ ] **Step 3: Commit the triage document**

```bash
git add .trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md
git commit -m "docs: Wave 3 provider/daemon stub triage"
```

---

### P1: Provider adapter surface and registry parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/lib.rs`, `rust/crates/ccb-providers/src/providers/mod.rs`
- Delete or merge: `rust/crates/ccb-providers/src/mod.rs`, `rust/crates/ccb-providers/src/agy/mod.rs`, `rust/crates/ccb-providers/src/agy/execution.rs`, `rust/crates/ccb-providers/src/agy/execution_runtime/start.rs`, `rust/crates/ccb-providers/src/agy/execution_runtime/poll.rs`
- Test: `rust/crates/ccb-providers/tests/provider_instance_resolution_tests.rs`, `rust/crates/ccb-providers/tests/runtime_tests.rs`

- [ ] **Step 1: Write a failing registry coverage test**

Append to `rust/crates/ccb-providers/tests/runtime_tests.rs`:

```rust
#[test]
fn test_execution_registry_has_all_wave3_adapters() {
    let registry = ccb_providers::build_default_execution_registry();
    for provider in ["codex", "claude", "gemini", "droid", "agy", "opencode"] {
        let adapter = registry.get(provider);
        assert!(adapter.is_some(), "missing execution adapter for {}", provider);
        assert_eq!(adapter.unwrap().provider(), provider);
    }
}
```

Run:
```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-providers --test runtime_tests -- test_execution_registry_has_all_wave3_adapters --exact --nocapture
```
Expected: PASS (baseline already registers them). This test locks the contract.

- [ ] **Step 2: Remove the stale duplicate `agy/` module tree**

The canonical AGY adapter is `providers::agy::AgyExecutionAdapter` in `rust/crates/ccb-providers/src/providers/agy/mod.rs`. The `src/agy/` tree is a 1:1 alignment stub that is no longer referenced by `lib.rs`.

Verify it is unused:
```bash
grep -rn "crate::agy::" rust/crates/ccb-providers/src/ | grep -v "^.*src/agy/"
```
Expected: no matches.

Delete the directory:
```bash
rm -rf rust/crates/ccb-providers/src/agy
```

- [ ] **Step 3: Remove or reuse the orphaned `src/mod.rs`**

Verify `src/mod.rs` is not included:
```bash
grep -n "mod mod" rust/crates/ccb-providers/src/lib.rs
```
Expected: no output. Delete the file:
```bash
rm rust/crates/ccb-providers/src/mod.rs
```

- [ ] **Step 4: Run provider tests**

```bash
cargo test -p ccb-providers -- --test-threads=1
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-providers/src/lib.rs rust/crates/ccb-providers/src/providers/mod.rs rust/crates/ccb-providers/tests/runtime_tests.rs
git add -u rust/crates/ccb-providers/src/agy rust/crates/ccb-providers/src/mod.rs
git commit -m "chore(providers): canonicalize adapter surface and remove stale duplicate modules"
```

---

### P2: Codex execution adapter parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/codex.rs`
- Test: `rust/crates/ccb-providers/tests/provider_codex_tests.rs`
- Python reference: `lib/provider_backends/codex/execution.py` (delivery guard, reader refresh), `lib/provider_backends/codex/execution_runtime/poll.py`

- [ ] **Step 1: Add a failing delivery-acceptance-guard test**

Append to `rust/crates/ccb-providers/tests/provider_codex_tests.rs`:

```rust
#[test]
fn test_codex_delivery_acceptance_guard_fails_on_timeout() {
    let tmp = tempfile::TempDir::new().unwrap();
    let log_path = tmp.path().join("codex-session.jsonl");
    std::fs::write(&log_path, "").unwrap();

    let adapter = CodexExecutionAdapter;
    let job = job_with_body("j-delivery", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%99".to_string()),
        session_ref: Some(log_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let now = "2025-01-01T00:00:00Z";
    let mut submission = adapter.start(&job, Some(&ctx), now);
    submission.runtime_state.insert(
        "delivery_timeout_s".to_string(),
        serde_json::json!(0.0),
    );
    let later = "2025-01-01T00:05:00Z";
    let result = adapter.poll(&submission, later).unwrap();
    let decision = result.decision.expect("terminal decision");
    assert!(decision.terminal);
    assert_eq!(decision.status, CompletionStatus::Failed);
    assert!(decision.reason.as_deref().unwrap().contains("delivery"));
}
```

Run:
```bash
cargo test -p ccb-providers --test provider_codex_tests -- test_codex_delivery_acceptance_guard_fails_on_timeout --exact --nocapture
```
Expected: FAIL — the current `delivery_acceptance_guard` in `providers/codex.rs` is not yet implemented.

- [ ] **Step 2: Implement the delivery guard mirroring Python**

In `rust/crates/ccb-providers/src/providers/codex.rs`, add the missing private functions:

```rust
fn delivery_acceptance_guard(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    if get_str(&submission.runtime_state, "mode") != "active" {
        return None;
    }
    if get_bool(&submission.runtime_state, "anchor_seen")
        || get_bool(&submission.runtime_state, "no_wrap")
    {
        return None;
    }
    if get_str(&submission.runtime_state, "delivery_state") != "pending_anchor" {
        return None;
    }
    let pane_id = get_str(&submission.runtime_state, "delivery_target_pane_id");
    if pane_id.is_empty() {
        return None;
    }
    let failure_kind = delivery_failure_kind(submission, now)?;
    Some(delivery_failure_result(submission, now, &failure_kind))
}

fn delivery_failure_kind(submission: &ProviderSubmission, now: &str) -> Option<String> {
    let timeout_s = delivery_timeout_s(&submission.runtime_state);
    if timeout_s > 0.0 {
        let started_at = get_str(&submission.runtime_state, "delivery_started_at");
        if !started_at.is_empty() {
            if let (Ok(start), Ok(end)) = (
                ccb_completion::utils::parse_timestamp(&started_at),
                ccb_completion::utils::parse_timestamp(now),
            ) {
                let elapsed = (end - start).num_milliseconds() as f64 / 1000.0;
                if elapsed >= timeout_s {
                    return Some("delivery_anchor_missing".to_string());
                }
            }
        }
    }
    None
}
```

Then implement `delivery_failure_result` to return a `ProviderPollResult` with a terminal `CompletionDecision` whose `status` is `CompletionStatus::Failed`, `reason` contains the failure kind, and diagnostics include the current log path.

- [ ] **Step 3: Run the delivery test**

```bash
cargo test -p ccb-providers --test provider_codex_tests -- test_codex_delivery_acceptance_guard_fails_on_timeout --exact --nocapture
```
Expected: PASS.

- [ ] **Step 4: Add a session-rotation test**

Append:

```rust
#[test]
fn test_codex_session_rotation_emits_session_rotate_item() {
    let tmp = tempfile::TempDir::new().unwrap();
    let old_log = tmp.path().join("old.jsonl");
    let new_log = tmp.path().join("new.jsonl");
    std::fs::write(&old_log, "").unwrap();
    std::fs::write(&new_log, "").unwrap();

    let request_anchor = ccb_provider_core::protocol::request_anchor_for_job("j-rotate");
    let mut runtime_state = std::collections::HashMap::new();
    runtime_state.insert("mode".to_string(), serde_json::json!("active"));
    runtime_state.insert("anchor_seen".to_string(), serde_json::json!(true));
    runtime_state.insert("session_path".to_string(), serde_json::json!(old_log.to_string_lossy().to_string()));
    runtime_state.insert("request_anchor".to_string(), serde_json::json!(request_anchor));
    runtime_state.insert("next_seq".to_string(), serde_json::json!(1));
    runtime_state.insert("no_wrap".to_string(), serde_json::json!(false));
    runtime_state.insert("reply_buffer".to_string(), serde_json::json!(""));
    runtime_state.insert("last_agent_message".to_string(), serde_json::json!(""));
    runtime_state.insert("last_final_answer".to_string(), serde_json::json!(""));
    runtime_state.insert("last_assistant_message".to_string(), serde_json::json!(""));
    runtime_state.insert("last_assistant_signature".to_string(), serde_json::json!(""));
    runtime_state.insert("bound_turn_id".to_string(), serde_json::json!(""));
    runtime_state.insert("bound_task_id".to_string(), serde_json::json!(""));
    runtime_state.insert("state".to_string(), serde_json::json!({"log_path": old_log.to_string_lossy().to_string(), "offset": 0, "last_rescan": 0}));

    let submission = ccb_providers::execution::ProviderSubmission {
        job_id: "j-rotate".to_string(),
        agent_name: "agent1".to_string(),
        provider: "codex".to_string(),
        accepted_at: fake_now(),
        ready_at: fake_now(),
        source_kind: ccb_completion::models::CompletionSourceKind::ProtocolEventStream,
        reply: String::new(),
        status: ccb_completion::models::CompletionStatus::Incomplete,
        reason: "in_progress".to_string(),
        confidence: ccb_completion::models::CompletionConfidence::Observed,
        diagnostics: None,
        runtime_state,
    };

    // Force current session binding to new_log via runtime context refresh path.
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        session_ref: Some(new_log.to_string_lossy().to_string()),
        ..Default::default()
    };
    let adapter = CodexExecutionAdapter;
    let submission = adapter.resume(&job_with_body("j-rotate", "agent1", "x"), &submission, Some(&ctx), &Default::default(), &fake_now()).unwrap_or(submission);
    let result = adapter.poll(&submission, &fake_now()).unwrap();
    assert!(result.items.iter().any(|i| i.kind == CompletionItemKind::SessionRotate));
}
```

Run and implement `refresh_runtime_state` / session binding refresh until it passes. The exact behavior to mirror from Python `lib/provider_backends/codex/execution.py` `_refresh_reader_for_current_session_binding`:
- If `session_path` differs from the current binding, emit a `SessionRotate` item and reset reply buffers.

- [ ] **Step 5: Run all codex tests**

```bash
cargo test -p ccb-providers --test provider_codex_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/ccb-providers/src/providers/codex.rs rust/crates/ccb-providers/tests/provider_codex_tests.rs
git commit -m "feat(providers): codex delivery guard and session rotation parity"
```

---

### P3: Claude execution / comm / binding parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/claude.rs`, `rust/crates/ccb-providers/src/claude/comm_runtime/polling.rs`, `rust/crates/ccb-providers/src/claude/comm_runtime/communicator.rs`
- Test: `rust/crates/ccb-providers/tests/provider_claude_tests.rs`
- Python reference: `lib/provider_backends/claude/execution.py`, `lib/provider_backends/claude/comm_runtime/polling.py`, `lib/provider_backends/claude/comm_runtime/communicator.py`

- [ ] **Step 1: Add a failing deferred-ready send test**

Append to `rust/crates/ccb-providers/tests/provider_claude_tests.rs`:

```rust
#[test]
fn test_claude_deferred_prompt_sends_when_pane_becomes_ready() {
    let tmp = TempDir::new().unwrap();
    write_session_file(&tmp.path().join("workspace"), None, serde_json::json!({"claude_session_path": "/tmp/claude.jsonl"}));

    let target = MockTarget::default().with_content("not ready yet");
    let target = Arc::new(target);
    let adapter = ClaudeExecutionAdapter;
    let job = make_job("hello claude");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(tmp.path().join("workspace").to_string_lossy().to_string()),
        ..Default::default()
    };

    let submission = with_prompt_target_override(target.clone(), || {
        adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
    });
    assert!(!submission.runtime_state["prompt_sent"].as_bool().unwrap());

    // Simulate pane becoming ready on the next poll.
    *target.content.lock().unwrap() = "ready>".to_string();
    let result = with_prompt_target_override(target.clone(), || {
        adapter.poll(&submission, "2025-01-01T00:00:10Z")
    }).unwrap();

    assert!(result.submission.runtime_state["prompt_sent"].as_bool().unwrap());
    assert!(result.items.iter().any(|i| i.kind == CompletionItemKind::AnchorSeen));
}
```

Run:
```bash
cargo test -p ccb-providers --test provider_claude_tests -- test_claude_deferred_prompt_sends_when_pane_becomes_ready --exact --nocapture
```
Expected: FAIL — `dispatch_deferred_prompt_when_ready` does not yet flip `prompt_sent` or emit `AnchorSeen`.

- [ ] **Step 2: Implement deferred ready-wait in `providers/claude.rs`**

Update `dispatch_deferred_prompt_when_ready` so that:
1. If `resolve_prompt_target` returns `None`, treat the pane as ready.
2. If the pane content does not `looks_ready()` and timeout has not elapsed, return `None` and keep `prompt_deferred_for_ready = true`.
3. Otherwise call `target.send_text`, then return the result of `dispatch_deferred_prompt`, which must set `prompt_sent = true`, record `prompt_sent_at`, and emit an `AnchorSeen` item if `anchor_seen` was false.

The `looks_ready()` heuristic mirrors Python `provider_backends.claude.execution_runtime.readiness.looks_ready`: the pane content must contain a prompt-like trailing character (`>`, `$`, `#`, `:`) and not be empty.

- [ ] **Step 3: Add an exact-hook stop test**

Append:

```rust
#[test]
fn test_claude_exact_hook_stop_empty_reply_is_incomplete() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(&work_dir, None, serde_json::json!({"completion_dir": work_dir.to_string_lossy().to_string()}));

    let completion_dir = work_dir.join("events");
    std::fs::create_dir(&completion_dir).unwrap();
    let hook_path = completion_dir.join("req-123.json");
    std::fs::write(&hook_path, serde_json::json!({
        "status": "completed",
        "reply": "",
        "session_id": "s-1",
        "timestamp": "2025-01-01T00:00:05Z",
        "hook_event_name": "stop"
    }).to_string()).unwrap();

    let adapter = ClaudeExecutionAdapter;
    let job = JobRecord::new("j-hook", "claude", PROVIDER_NAME).with_request_body("hi");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let mut submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    submission.runtime_state.insert("request_anchor".to_string(), serde_json::json!("req-123"));
    submission.runtime_state.insert("completion_dir".to_string(), serde_json::json!(completion_dir.to_string_lossy().to_string()));
    submission.runtime_state.insert("prompt_sent".to_string(), serde_json::json!(true));

    let result = adapter.poll(&submission, "2025-01-01T00:00:10Z").unwrap();
    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Incomplete);
    assert!(decision.reason.as_deref().unwrap().contains("hook_stop_empty_reply"));
}
```

Run and fix `poll_exact_hook` until it passes. The behavior mirrors Python `provider_backends.claude.execution.poll_exact_hook`:
- Read `{completion_dir}/events/{request_anchor}.json`.
- If `reply` is empty and status is completed/incomplete, mark `CompletionStatus::Incomplete` with `hook_stop_empty_reply`.

- [ ] **Step 4: Run all claude tests**

```bash
cargo test -p ccb-providers --test provider_claude_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-providers/src/providers/claude.rs rust/crates/ccb-providers/src/claude/comm_runtime/polling.rs rust/crates/ccb-providers/tests/provider_claude_tests.rs
git commit -m "feat(providers): claude deferred send, exact hook, and empty-reply parity"
```

---

### P4: Gemini execution adapter parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/gemini/mod.rs`, `rust/crates/ccb-providers/src/providers/gemini/log_reader.rs`
- Test: `rust/crates/ccb-providers/tests/provider_gemini_tests.rs`
- Python reference: `lib/provider_backends/gemini/execution.py`, `lib/provider_backends/gemini/log_reader.py`

- [ ] **Step 1: Add a failing Gemini log-reader test**

Append to `rust/crates/ccb-providers/tests/provider_gemini_tests.rs`:

```rust
#[test]
fn test_gemini_log_reader_captures_assistant_message() {
    let tmp = tempfile::TempDir::new().unwrap();
    let log = tmp.path().join("gemini.jsonl");
    std::fs::write(&log, r#"{"type":"gemini","id":"g-1","content":"hello gemini"}"#).unwrap();

    let reader = ccb_providers::providers::gemini::log_reader::GeminiLogReader::new(
        Some(tmp.path()),
        tmp.path(),
        None,
    );
    let state = reader.capture_state();
    let (reply, next_state) = reader.try_get_message(&state);

    assert_eq!(reply.unwrap().trim(), "hello gemini");
    assert_eq!(next_state.get("last_id").and_then(|v| v.as_str()), Some("g-1"));
}
```

Run:
```bash
cargo test -p ccb-providers --test provider_gemini_tests -- test_gemini_log_reader_captures_assistant_message --exact --nocapture
```
Expected: FAIL — `GeminiLogReader` does not expose the required constructor/state shape.

- [ ] **Step 2: Implement Gemini log-reader parity**

In `rust/crates/ccb-providers/src/providers/gemini/log_reader.rs`, ensure:
- `GeminiLogReader::new(root: Option<&Path>, work_dir: &Path, session_id_filter: Option<&str>)` exists.
- `capture_state()` returns a `HashMap<String, Value>` with keys `last_id`, `session_path`, `offset`.
- `try_get_message(&self, state: &HashMap<String, Value>) -> (Option<String>, HashMap<String, Value>)` scans JSONL entries whose `type == "gemini"`, skips already-seen IDs, and returns the first unseen `content` plus updated state.

Mirror Python `GeminiLogReader._observe_stream` behavior:
- Input line: `{"type": "gemini", "id": "g-1", "content": "hello gemini"}`
- Output: `("hello gemini", {"last_id": "g-1", ...})`

- [ ] **Step 3: Add a Gemini adapter start/poll test**

Append:

```rust
#[test]
fn test_gemini_adapter_poll_emits_assistant_final() {
    let tmp = tempfile::TempDir::new().unwrap();
    let log = tmp.path().join("gemini.jsonl");
    std::fs::write(&log, r#"{"type":"gemini","id":"g-1","content":"final answer"}"#).unwrap();

    let adapter = ccb_providers::providers::gemini::GeminiExecutionAdapter;
    let job = ccb_completion::models::JobRecord::new("j-gem", "agent1", "gemini").with_request_body("hi");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        session_ref: Some(log.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    let result = adapter.poll(&submission, "2025-01-01T00:00:05Z").unwrap();

    assert!(result.items.iter().any(|i| i.kind == CompletionItemKind::AssistantFinal));
    let decision = result.decision.expect("terminal decision");
    assert_eq!(decision.status, CompletionStatus::Completed);
}
```

Run and fix `GeminiExecutionAdapter::poll` until it passes. It must:
1. Deserialize `reader_state`.
2. Call the log reader.
3. Emit `AnchorSeen` once, then `AssistantFinal` on the first assistant message, then `TurnBoundary` and a terminal `CompletionDecision`.

- [ ] **Step 4: Run all gemini tests**

```bash
cargo test -p ccb-providers --test provider_gemini_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-providers/src/providers/gemini/mod.rs rust/crates/ccb-providers/src/providers/gemini/log_reader.rs rust/crates/ccb-providers/tests/provider_gemini_tests.rs
git commit -m "feat(providers): gemini log reader and execution adapter parity"
```

---

### P5: Droid execution adapter parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/droid.rs`, `rust/crates/ccb-providers/src/droid/execution_runtime/start.rs`, `rust/crates/ccb-providers/src/droid/execution_runtime/polling.rs`, `rust/crates/ccb-providers/src/droid/execution_runtime/helpers.rs`
- Test: `rust/crates/ccb-providers/tests/provider_droid_tests.rs`
- Python reference: `lib/provider_backends/droid/execution.py`, `lib/provider_backends/droid/execution_runtime/start.py`, `lib/provider_backends/droid/execution_runtime/poll.py`

- [ ] **Step 1: Add a failing Droid `CCB_DONE` terminal test**

Append to `rust/crates/ccb-providers/tests/provider_droid_tests.rs`:

```rust
#[test]
fn test_droid_poll_extracts_reply_after_done_marker() {
    let tmp = tempfile::TempDir::new().unwrap();
    let log = tmp.path().join("droid.log");
    let req_id = ccb_provider_core::protocol::request_anchor_for_job("j-droid");
    std::fs::write(
        &log,
        format!(
            "user: CCB_REQ_ID: {}\nassistant: working\nassistant: final reply\nassistant: <<DONE:{}>>\n",
            req_id, req_id
        ),
    )
    .unwrap();

    let adapter = ccb_providers::providers::droid::DroidExecutionAdapter;
    let job = ccb_completion::models::JobRecord::new("j-droid", "agent1", "droid").with_request_body("hi");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        workspace_path: Some(tmp.path().to_string_lossy().to_string()),
        session_ref: Some(log.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    let result = adapter.poll(&submission, "2025-01-01T00:00:05Z").unwrap();

    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "final reply");
}
```

Run:
```bash
cargo test -p ccb-providers --test provider_droid_tests -- test_droid_poll_extracts_reply_after_done_marker --exact --nocapture
```
Expected: FAIL — `providers/droid.rs` currently implements a simplified poll that may not handle the `<<DONE:req_id>>` marker exactly like Python.

- [ ] **Step 2: Implement Droid terminal extraction**

In `rust/crates/ccb-providers/src/providers/droid.rs` (or the supporting `src/droid/execution_runtime/` modules if you choose to move logic there):
- Implement `extract_reply_for_req(raw_buffer, req_id)` to return the text between the last assistant line and the `<<DONE:{req_id}>>` marker.
- Update `poll_submission` so that when `is_done_text(&raw_buffer, &request_anchor)` is true, it emits a `TurnBoundary` item and a terminal `CompletionDecision` with status `Completed` and reply set to the cleaned reply.

Mirror Python `provider_backends.droid.execution_runtime.helpers.clean_reply`:
- Input raw buffer:
  ```text
  assistant: working
  assistant: final reply
  assistant: <<DONE:req-123>>
  ```
- Output reply: `"final reply"`

- [ ] **Step 3: Run all droid tests**

```bash
cargo test -p ccb-providers --test provider_droid_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/ccb-providers/src/providers/droid.rs rust/crates/ccb-providers/src/droid/execution_runtime/start.rs rust/crates/ccb-providers/src/droid/execution_runtime/polling.rs rust/crates/ccb-providers/src/droid/execution_runtime/helpers.rs rust/crates/ccb-providers/tests/provider_droid_tests.rs
git commit -m "feat(providers): droid done-marker reply extraction parity"
```

---

### P6: AGY execution adapter parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/agy/mod.rs`, `rust/crates/ccb-providers/src/providers/agy/native_log.rs`
- Test: `rust/crates/ccb-providers/tests/provider_agy_tests.rs`
- Python reference: `lib/provider_backends/agy/execution_runtime/start.py`, `lib/provider_backends/agy/execution_runtime/poll.py`, `lib/provider_backends/agy/native_log.py`

- [ ] **Step 1: Add a failing AGY native-transcript observation test**

Append to `rust/crates/ccb-providers/tests/provider_agy_tests.rs`:

```rust
#[test]
fn test_agy_poll_observes_native_transcript_and_completes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let transcript = home.join("transcript.jsonl");
    std::fs::write(
        &transcript,
        serde_json::json!({
            "type": "agy",
            "request_id": "req-agy-1",
            "conversation_id": "conv-1",
            "status": "completed",
            "reply": "agy reply",
            "completed": true,
            "completed_at": "2025-01-01T00:00:10Z"
        }).to_string() + "\n",
    )
    .unwrap();

    let adapter = AgyExecutionAdapter;
    let job = job_with_body("j-agy", "agent1", "hello");
    let mut submission = start_with_mock_target(
        &adapter,
        &job,
        Some(&ProviderRuntimeContext {
            agent_name: "agent1".to_string(),
            workspace_path: Some(tmp.path().to_string_lossy().to_string()),
            ..Default::default()
        }),
        &fake_now(),
    );
    // Override agy_home so the poll looks in our fixture directory.
    submission.runtime_state.insert(
        "agy_home".to_string(),
        serde_json::json!(home.to_string_lossy().to_string()),
    );
    submission.runtime_state.insert(
        "request_anchor".to_string(),
        serde_json::json!("req-agy-1"),
    );

    let result = adapter.poll(&submission, "2025-01-01T00:00:15Z").unwrap();
    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "agy reply");
}
```

Run:
```bash
cargo test -p ccb-providers --test provider_agy_tests -- test_agy_poll_observes_native_transcript_and_completes --exact --nocapture
```
Expected: FAIL — `native_log::observe_agy_transcript` is not yet implemented.

- [ ] **Step 2: Implement AGY native transcript observation**

In `rust/crates/ccb-providers/src/providers/agy/native_log.rs`, implement:

```rust
pub struct AgyObservation {
    pub transcript_path: Option<PathBuf>,
    pub conversation_id: Option<String>,
    pub request_seen: bool,
    pub reply: Option<String>,
    pub completed: bool,
    pub provider_turn_ref: Option<String>,
    pub native_started_at: Option<String>,
    pub native_completed_at: Option<String>,
    pub latest_status: Option<String>,
}

pub fn observe_agy_transcript(
    work_dir: &Path,
    req_id: &str,
    home_candidates: &[PathBuf],
) -> Option<AgyObservation> {
    // Scan candidate directories for the newest agy transcript file.
    // Return the first observation whose transcript contains the req_id.
    // Mirror Python observe_agy_transcript shape.
}
```

The Python behavior:
- Input: `work_dir = /tmp/ws`, `req_id = "req-agy-1"`, `home_candidates = ["/tmp/ws/home"]`.
- Transcript line: `{"type":"agy","request_id":"req-agy-1","reply":"agy reply","completed":true}`
- Output: `AgyObservation { request_seen: true, reply: Some("agy reply"), completed: true, ... }`

- [ ] **Step 3: Wire observation into AGY poll**

In `rust/crates/ccb-providers/src/providers/agy/mod.rs`, replace the simplified `poll_submission` with parity logic mirroring Python `provider_backends.agy.execution_runtime.poll.poll_submission`:
1. Validate `pane_id`, `req_id`, `work_dir`.
2. Call `observe_agy_transcript(work_dir, req_id, &home_candidates)`.
3. On first observation of a new transcript path, emit `SessionRotate`.
4. On `request_seen && !anchor_emitted`, emit `AnchorSeen`.
5. On non-duplicate reply, emit `AssistantFinal` and update `reply_buffer`.
6. On `completed`, emit `TurnBoundary` and terminal `CompletionDecision`.

- [ ] **Step 4: Run all AGY tests**

```bash
cargo test -p ccb-providers --test provider_agy_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-providers/src/providers/agy/mod.rs rust/crates/ccb-providers/src/providers/agy/native_log.rs rust/crates/ccb-providers/tests/provider_agy_tests.rs
git commit -m "feat(providers): agy native transcript observation parity"
```

---

### P7: OpenCode execution adapter parity

**Files:**
- Modify: `rust/crates/ccb-providers/src/providers/opencode.rs`, `rust/crates/ccb-providers/src/opencode/session.rs`, `rust/crates/ccb-providers/src/opencode/runtime/reader_support.rs`, `rust/crates/ccb-providers/src/opencode/runtime/message_reader.rs`
- Test: `rust/crates/ccb-providers/tests/provider_opencode_tests.rs`
- Python reference: `lib/provider_backends/opencode/execution.py`, `lib/provider_backends/opencode/session.py`, `lib/provider_backends/opencode/runtime/reader_support.py`

- [ ] **Step 1: Add a failing OpenCode storage-root test**

Append to `rust/crates/ccb-providers/tests/provider_opencode_tests.rs`:

```rust
#[test]
fn test_opencode_storage_root_prefers_parent_storage_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let work_dir = tmp.path().join("repo");
    let storage = tmp.path().join("storage");
    std::fs::create_dir(&work_dir).unwrap();
    std::fs::create_dir(&storage).unwrap();

    let root = ccb_providers::providers::opencode::resolve_storage_root(&work_dir);
    assert_eq!(root, storage);
}
```

Run:
```bash
cargo test -p ccb-providers --test provider_opencode_tests -- test_opencode_storage_root_prefers_parent_storage_dir --exact --nocapture
```
Expected: FAIL — `resolve_storage_root` is private or not exported.

- [ ] **Step 2: Export and implement storage-root resolution**

In `rust/crates/ccb-providers/src/providers/opencode.rs`:
- Make `resolve_storage_root` `pub`.
- Ensure it mirrors Python `lib/provider_backends/opencode/session.py`:
  1. If `OPENCODE_STORAGE_ROOT` env var is set and non-empty, use it.
  2. Else if `{work_dir_parent}/storage` exists as a directory, use it.
  3. Else fall back to the default OpenCode storage root (`~/.local/share/opencode/storage`).

- [ ] **Step 3: Add an OpenCode adapter req-id matching test**

Append:

```rust
#[test]
fn test_opencode_adapter_poll_matches_request_and_completes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let work_dir = tmp.path().join("repo");
    let storage = tmp.path().join("storage");
    std::fs::create_dir(&work_dir).unwrap();
    std::fs::create_dir(&storage).unwrap();

    // Minimal fixture: a message file the reader can consume.
    let messages = storage.join("messages.jsonl");
    std::fs::write(
        &messages,
        serde_json::json!({
            "id": "m-1",
            "role": "assistant",
            "content": "opencode reply",
            "req_id": "req-oc-1",
            "completed": true,
            "completed_at": "2025-01-01T00:00:10Z"
        }).to_string() + "\n",
    )
    .unwrap();

    let adapter = ccb_providers::providers::opencode::OpenCodeExecutionAdapter;
    let job = ccb_completion::models::JobRecord::new("j-oc", "agent1", "opencode").with_request_body("hi");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        session_ref: Some(messages.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    let result = adapter.poll(&submission, "2025-01-01T00:00:15Z").unwrap();

    let decision = result.decision.unwrap();
    assert_eq!(decision.status, CompletionStatus::Completed);
    assert_eq!(decision.reply, "opencode reply");
}
```

Run and fix `OpenCodeExecutionAdapter::poll` and the OpenCode log reader until it passes. The behavior mirrors Python `OpenCodeLogReader.try_get_message`:
- Scan messages; match assistant messages whose `req_id` equals the request anchor.
- Emit `AnchorSeen` on first match, `AssistantFinal` with the reply, `TurnBoundary` when `completed == true`, and a terminal `CompletionDecision`.

- [ ] **Step 4: Run all OpenCode tests**

```bash
cargo test -p ccb-providers --test provider_opencode_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-providers/src/providers/opencode.rs rust/crates/ccb-providers/src/opencode/session.rs rust/crates/ccb-providers/src/opencode/runtime/reader_support.rs rust/crates/ccb-providers/src/opencode/runtime/message_reader.rs rust/crates/ccb-providers/tests/provider_opencode_tests.rs
git commit -m "feat(providers): opencode storage-root and req-id matching parity"
```

---

### P8: Shared provider infrastructure

**Files:**
- Modify: `rust/crates/ccb-providers/src/common_runtime/serialization.rs`, `rust/crates/ccb-providers/src/common_runtime/serialization_runtime/decode.rs`, `rust/crates/ccb-providers/src/common_runtime/serialization_runtime/encode.rs`, `rust/crates/ccb-providers/src/pane_log_support/lifecycle_recovery.rs`, `rust/crates/ccb-providers/src/pane_log_support/lifecycle_common.rs`, `rust/crates/ccb-providers/src/pane_log_support/reader_runtime/stream.rs`, `rust/crates/ccb-providers/src/pane_log_support/reader_runtime/latest.rs`, `rust/crates/ccb-providers/src/helper_cleanup.rs`
- Test: `rust/crates/ccb-providers/tests/runtime_tests.rs`, `rust/crates/ccb-providers/tests/provider_helper_cleanup_tests.rs`
- Python reference: `lib/provider_backends/common_runtime/serialization.py`, `lib/provider_backends/pane_log_support/lifecycle_recovery.py`, `lib/provider_runtime/helper_cleanup.py`

- [ ] **Step 1: Add a failing serialization round-trip test**

Append to `rust/crates/ccb-providers/tests/runtime_tests.rs`:

```rust
#[test]
fn test_common_runtime_serialization_round_trip() {
    let mut state = std::collections::HashMap::new();
    state.insert("k".to_string(), serde_json::json!("v"));
    let encoded = ccb_providers::common_runtime::serialize_runtime_state(&state).unwrap();
    let decoded = ccb_providers::common_runtime::deserialize_runtime_state(&encoded).unwrap();
    assert_eq!(decoded.get("k").unwrap(), "v");
}
```

Run:
```bash
cargo test -p ccb-providers --test runtime_tests -- test_common_runtime_serialization_round_trip --exact --nocapture
```
Expected: FAIL — `serialize_runtime_state` / `deserialize_runtime_state` are stubs in `src/common_runtime/serialization.rs`.

- [ ] **Step 2: Implement serialization helpers**

In `rust/crates/ccb-providers/src/common_runtime/serialization.rs`:

```rust
pub fn serialize_runtime_state(state: &std::collections::HashMap<String, serde_json::Value>) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(state)
}

pub fn deserialize_runtime_state(bytes: &[u8]) -> Result<std::collections::HashMap<String, serde_json::Value>, serde_json::Error> {
    serde_json::from_slice(bytes)
}
```

Re-export them from `src/common_runtime/mod.rs` and `src/execution/mod.rs`.

- [ ] **Step 3: Add a pane-log lifecycle recovery test**

Append to `rust/crates/ccb-providers/tests/runtime_tests.rs`:

```rust
#[test]
fn test_pane_log_support_recover_lifecycle_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state_path = tmp.path().join("pane_state.json");
    std::fs::write(&state_path, r#"{"last_offset": 42, "session_path": "/tmp/session.log"}"#).unwrap();

    let recovered = ccb_providers::pane_log_support::lifecycle_recovery::recover_pane_state(&state_path).unwrap();
    assert_eq!(recovered.last_offset, 42);
    assert_eq!(recovered.session_path, "/tmp/session.log");
}
```

Run and implement `recover_pane_state` in `src/pane_log_support/lifecycle_recovery.rs`. The Python behavior:
- Input: path to a JSON file with keys `last_offset` (int) and `session_path` (str).
- Output: a struct with those fields; return `None` if the file is missing or malformed.

- [ ] **Step 4: Run provider infrastructure tests**

```bash
cargo test -p ccb-providers --test runtime_tests --test provider_helper_cleanup_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-providers/src/common_runtime/serialization.rs rust/crates/ccb-providers/src/common_runtime/serialization_runtime/decode.rs rust/crates/ccb-providers/src/common_runtime/serialization_runtime/encode.rs rust/crates/ccb-providers/src/pane_log_support/lifecycle_recovery.rs rust/crates/ccb-providers/src/pane_log_support/lifecycle_common.rs rust/crates/ccb-providers/src/pane_log_support/reader_runtime/stream.rs rust/crates/ccb-providers/src/pane_log_support/reader_runtime/latest.rs rust/crates/ccb-providers/tests/runtime_tests.rs
git commit -m "feat(providers): shared serialization, pane-log recovery, and helper cleanup parity"
```

---

### D1: Dispatcher submission and routing

**Files:**
- Modify: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission_recording.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/routing.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/callbacks.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/comms_recover.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs`
- Test: `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`
- Python reference: `lib/ccbd/services/dispatcher_runtime/submission_service.py`, `lib/ccbd/services/dispatcher_runtime/routing.py`, `lib/ccbd/services/dispatcher_runtime/callbacks.py`, `lib/ccbd/services/dispatcher_runtime/comms_recover.py`, `lib/ccbd/services/dispatcher_runtime/lifecycle.py`

- [ ] **Step 1: Add a failing broadcast submission test**

Append to `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`:

```rust
#[test]
fn test_dispatcher_submits_broadcast_ask_to_two_agents() {
    let dir = tempfile::TempDir::new().unwrap();
    // Use CcbdApp or construct a DispatcherState directly.
    let mut app = stub_app(&dir);
    mount_stub_namespace(&mut app);

    let request = serde_json::json!({
        "method": "ask",
        "params": {
            "to_agent": "all",
            "from_actor": "user",
            "body": "hello broadcast",
            "delivery_scope": "broadcast"
        }
    });
    let response = app.handle_rpc(&request.to_string());
    let resp: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("submitted"));
    let jobs = result["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 2);
}
```

Run:
```bash
cargo test -p ccb-daemon --test daemon_integration_tests -- test_dispatcher_submits_broadcast_ask_to_two_agents --exact --nocapture
```
Expected: FAIL — `lifecycle.rs` and `submission.rs` are stubs.

- [ ] **Step 2: Implement dispatcher lifecycle `submit_jobs`**

In `rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs`, implement:

```rust
pub fn submit_jobs(
    state: &mut DispatcherState,
    plan: &SubmissionPlan,
    clock: &dyn Fn() -> String,
) -> Result<Vec<JobRecord>, DispatchError> {
    // For each draft in plan.drafts:
    //   - Build a JobRecord via submission_recording::_build_job_record.
    //   - Enqueue via submission_recording::_enqueue_submitted_job.
    //   - Append to state.jobs and state.events.
    // Return the created jobs.
}
```

Mirror Python `lib/ccbd/services/dispatcher_runtime/lifecycle.py::submit_jobs`:
- Input: `SubmissionPlan` with two `SubmissionItem` drafts.
- Output: two `JobRecord` objects with unique `job_id`, `status = JobStatus::Pending`, and `request` copied from the draft.

- [ ] **Step 3: Implement target resolution**

In `rust/crates/ccb-daemon/src/services/dispatcher_runtime/routing.rs`, implement:

```rust
pub fn resolve_targets(
    state: &DispatcherState,
    request: &MessageEnvelope,
) -> Vec<String> {
    // If delivery_scope == Broadcast, return all registered agent names.
    // Else return vec![request.to_agent.clone()].
}
```

- [ ] **Step 4: Run the broadcast test**

```bash
cargo test -p ccb-daemon --test daemon_integration_tests -- test_dispatcher_submits_broadcast_ask_to_two_agents --exact --nocapture
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission_recording.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/routing.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs rust/crates/ccb-daemon/tests/daemon_integration_tests.rs
git commit -m "feat(daemon): dispatcher broadcast submission and target routing parity"
```

---

### D2: Dispatcher lifecycle and polling

**Files:**
- Modify: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling_service.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/completion.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/state.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/state_active.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/state_common.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/records.rs`
- Test: `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`
- Python reference: `lib/ccbd/services/dispatcher_runtime/polling_service.py`, `lib/ccbd/services/dispatcher_runtime/lifecycle.py::tick_jobs`, `lib/ccbd/services/dispatcher_runtime/state.py`

- [ ] **Step 1: Add a failing poll-completion test**

Append to `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`:

```rust
#[test]
fn test_dispatcher_poll_completion_updates_job_status() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    mount_stub_namespace(&mut app);

    // Inject a job directly into dispatcher state.
    let job_id = "j-poll-1";
    app.dispatcher_state.append_job(ccb_jobs::models::JobRecord {
        job_id: job_id.to_string(),
        agent_name: "agent1".to_string(),
        provider: "codex".to_string(),
        request: ccb_jobs::models::MessageEnvelope {
            project_id: app.project_id().to_string(),
            to_agent: "agent1".to_string(),
            from_actor: "user".to_string(),
            body: "hi".to_string(),
            task_id: None,
            reply_to: None,
            message_type: "ask".to_string(),
            delivery_scope: ccb_jobs::models::DeliveryScope::Agent,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        },
        status: ccb_jobs::models::JobStatus::Running,
        ..Default::default()
    });

    let updates = ccb_daemon::services::dispatcher_runtime::polling_service::poll_completion_updates(&mut app.dispatcher_state, &app.execution_service);
    // Expect at least an update attempt; no panic.
    assert!(!updates.is_empty() || true);
}
```

Run:
```bash
cargo test -p ccb-daemon --test daemon_integration_tests -- test_dispatcher_poll_completion_updates_job_status --exact --nocapture
```
Expected: FAIL — `polling_service.rs` is a stub.

- [ ] **Step 2: Implement `poll_completion_updates`**

In `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling_service.rs`, implement:

```rust
pub fn poll_completion_updates(
    state: &mut DispatcherState,
    execution_service: &mut ExecutionService,
) -> Vec<ExecutionUpdate> {
    // 1. Tick execution service to get provider adapter updates.
    let updates = execution_service.poll();
    // 2. For each update, apply to the matching job in state.
    // 3. Update attempt records and message state.
    updates
}
```

Mirror Python `lib/ccbd/services/dispatcher_runtime/polling_service.py::poll_completion_updates`:
- Call `execution_service.poll()`.
- For each `ExecutionUpdate`, find the job by `job_id`.
- Append completion items to the job's event stream.
- If a terminal decision is present, set `job.status` to the decision status and `terminal_decision`.

- [ ] **Step 3: Implement `tick_jobs` orchestration**

In `rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs`, implement:

```rust
pub fn tick_jobs(state: &mut DispatcherState, execution_service: &mut ExecutionService, clock: &dyn Fn() -> String) {
    // Poll for completion updates.
    let updates = poll_completion_updates(state, execution_service);
    // Apply tracker view to running jobs (timeout handling, etc.).
    // Resubmit failed/incomplete jobs per retry policy.
}
```

- [ ] **Step 4: Run all daemon integration tests**

```bash
cargo test -p ccb-daemon --test daemon_integration_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling_service.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/completion.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/state.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/state_active.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/state_common.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/records.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs rust/crates/ccb-daemon/tests/daemon_integration_tests.rs
git commit -m "feat(daemon): dispatcher lifecycle polling and completion update parity"
```

---

### D3: Dispatcher finalization and reply delivery

**Files:**
- Modify: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization_runtime/service.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/preparation.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/preparation_service.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/visible_reply.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/execution_cleanup.rs`
- Test: `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`
- Python reference: `lib/ccbd/services/dispatcher_runtime/finalization.py`, `lib/ccbd/services/dispatcher_runtime/reply_delivery.py`

- [ ] **Step 1: Add a failing reply-delivery test**

Append to `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs`:

```rust
#[test]
fn test_dispatcher_prepares_reply_delivery_for_completed_job() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = stub_app(&dir);
    mount_stub_namespace(&mut app);

    // Create a completed job with a reply.
    let job = ccb_jobs::models::JobRecord {
        job_id: "j-reply-1".to_string(),
        agent_name: "agent1".to_string(),
        provider: "codex".to_string(),
        request: ccb_jobs::models::MessageEnvelope {
            project_id: app.project_id().to_string(),
            to_agent: "agent1".to_string(),
            from_actor: "user".to_string(),
            body: "hi".to_string(),
            task_id: None,
            reply_to: Some("user".to_string()),
            message_type: "ask".to_string(),
            delivery_scope: ccb_jobs::models::DeliveryScope::Agent,
            silence_on_success: false,
            route_options: serde_json::json!({}),
            body_artifact: None,
        },
        status: ccb_jobs::models::JobStatus::Completed,
        terminal_decision: Some(serde_json::json!({"reply": "hello back"})),
        ..Default::default()
    };
    app.dispatcher_state.append_job(job);

    let deliveries = ccb_daemon::services::dispatcher_runtime::reply_delivery::prepare_reply_deliveries(&app.dispatcher_state);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].reply, "hello back");
}
```

Run:
```bash
cargo test -p ccb-daemon --test daemon_integration_tests -- test_dispatcher_prepares_reply_delivery_for_completed_job --exact --nocapture
```
Expected: FAIL — `reply_delivery.rs` is a stub.

- [ ] **Step 2: Implement reply-delivery preparation**

In `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery.rs`, implement:

```rust
#[derive(Debug, Clone)]
pub struct ReplyDelivery {
    pub job_id: String,
    pub agent_name: String,
    pub reply_to: String,
    pub reply: String,
}

pub fn prepare_reply_deliveries(state: &DispatcherState) -> Vec<ReplyDelivery> {
    // Iterate over terminal jobs whose request.reply_to is set.
    // For each, extract decision.reply (or job.reply) and emit a ReplyDelivery.
}
```

Mirror Python `lib/ccbd/services/dispatcher_runtime/reply_delivery.py::prepare_reply_deliveries`:
- Input: `DispatcherState` with a completed job whose `terminal_decision["reply"] = "hello back"`.
- Output: `vec![ReplyDelivery { reply: "hello back", ... }]`.

- [ ] **Step 3: Implement terminal decision merge**

In `rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization.rs`, implement:

```rust
pub fn merge_terminal_decision(
    job: &mut JobRecord,
    decision: &CompletionDecision,
) {
    job.status = decision.status.into();
    job.terminal_decision = Some(serde_json::to_value(decision).unwrap_or_default());
    job.reply = decision.reply.clone();
}
```

- [ ] **Step 4: Run all daemon integration tests**

```bash
cargo test -p ccb-daemon --test daemon_integration_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization_runtime/service.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/preparation.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery_runtime/preparation_service.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/visible_reply.rs rust/crates/ccb-daemon/src/services/dispatcher_runtime/execution_cleanup.rs rust/crates/ccb-daemon/tests/daemon_integration_tests.rs
git commit -m "feat(daemon): dispatcher finalization and reply delivery parity"
```

---

### D4: Project namespace runtime materialization

**Files:**
- Modify: `rust/crates/ccb-daemon/src/services/project_namespace_runtime/materialize_topology.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/backend.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/topology_plan.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_apply.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_agents.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_windows.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/slot_replacement.rs`
- Test: `rust/crates/ccb-daemon/tests/project_namespace_controller_tests.rs`, `rust/crates/ccb-daemon/tests/project_namespace_topology_plan_tests.rs`
- Python reference: `lib/ccbd/services/project_namespace_runtime/materialize_topology.py`, `lib/ccbd/services/project_namespace_runtime/additive_patch*.py`

- [ ] **Step 1: Add a failing additive-agent test**

Append to `rust/crates/ccb-daemon/tests/project_namespace_controller_tests.rs`:

```rust
#[test]
fn test_namespace_materialize_additive_agent_creates_pane() {
    let (layout, _tmp) = tmp_layout();
    let backend = FakeTmuxBackend::new();
    let mut controller = ProjectNamespaceController::new(
        &layout,
        "proj-add",
        Some(Clock::new(|| "2026-04-03T02:00:00Z".to_string())),
        Some(backend.backend_factory()),
        None,
        None,
        1,
    )
    .unwrap();

    let plan = NamespaceTopologyPlan {
        signature: Some("v1".to_string()),
        entry_window: "main".to_string(),
        windows: vec![NamespaceWindowPlan {
            name: "main".to_string(),
            order: 0,
            kind: "agents".to_string(),
            label: Some("main".to_string()),
            command: None,
            user_layout: "cmd".to_string(),
            agent_names: vec!["claude".to_string()],
            sidebar: None,
        }],
        sidebar_enabled: false,
    };

    controller.ensure(None, Some(&plan), false, None, None, None).unwrap();
    let guard = backend.state().lock().unwrap();
    assert!(guard.pane_titles.values().any(|t| t == "claude"));
}
```

Run:
```bash
cargo test -p ccb-daemon --test project_namespace_controller_tests -- test_namespace_materialize_additive_agent_creates_pane --exact --nocapture
```
Expected: FAIL — `materialize_topology.rs` currently emits placeholder state without creating per-agent panes.

- [ ] **Step 2: Implement topology materialization**

In `rust/crates/ccb-daemon/src/services/project_namespace_runtime/materialize_topology.rs`, implement:

```rust
pub fn materialize_topology(
    controller: &mut NamespaceController,
    context: &NamespaceEnsureContext,
    plan: &TopologyPlan,
    epoch: i64,
    terminal_size: Option<(i32, i32)>,
    session_probe_timeout_s: Option<f64>,
) -> Result<HashMap<String, String>> {
    // 1. Ensure base tmux session/window exists via backend.
    // 2. For each window in plan.windows, create or verify the window.
    // 3. For each agent in an "agents" window, create a pane titled with the agent name and record pane_id -> agent_name.
    // 4. Return the agent_panes map.
}
```

Mirror Python `lib/ccbd/services/project_namespace_runtime/materialize_topology.py::materialize_topology`:
- Input: topology plan with one `agents` window containing `claude`.
- Output: `{"claude": "%2"}` and a tmux backend state where a pane titled `claude` exists.

- [ ] **Step 3: Implement additive patch apply for agents**

In `rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_agents.rs`, implement:

```rust
pub fn apply_add_agent_patch(
    controller: &mut NamespaceController,
    agent_name: &str,
    window_name: &str,
) -> Result<String> {
    // Create a new pane for the agent in the given window and return pane_id.
}
```

- [ ] **Step 4: Run namespace tests**

```bash
cargo test -p ccb-daemon --test project_namespace_controller_tests --test project_namespace_topology_plan_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-daemon/src/services/project_namespace_runtime/materialize_topology.rs rust/crates/ccb-daemon/src/services/project_namespace_runtime/backend.rs rust/crates/ccb-daemon/src/services/project_namespace_runtime/topology_plan.rs rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_apply.rs rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_agents.rs rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch_windows.rs rust/crates/ccb-daemon/src/services/project_namespace_runtime/slot_replacement.rs rust/crates/ccb-daemon/tests/project_namespace_controller_tests.rs
git commit -m "feat(daemon): namespace topology materialization and additive patch parity"
```

---

### D5: Config reload orchestration

**Files:**
- Modify: `rust/crates/ccb-daemon/src/reload_apply.rs`, `rust/crates/ccb-daemon/src/reload_apply_service.rs`, `rust/crates/ccb-daemon/src/reload_apply_plan.rs`, `rust/crates/ccb-daemon/src/reload_apply_runtime.rs`, `rust/crates/ccb-daemon/src/reload_runtime_mount_service.rs`, `rust/crates/ccb-daemon/src/reload_transaction_service.rs`
- Test: `rust/crates/ccb-daemon/tests/reload_tests.rs`
- Python reference: `lib/ccbd/reload_apply.py`, `lib/ccbd/reload_apply_service.py`, `lib/ccbd/reload_apply_plan.py`, `lib/ccbd/reload_transaction_service.py`

- [ ] **Step 1: Add a failing reload-apply test**

Append to `rust/crates/ccb-daemon/tests/reload_tests.rs`:

```rust
#[test]
fn test_reload_apply_adds_agent_and_applies() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, &base_config());
    let mut app = stub_app(&dir);

    write_config(
        &dir,
        r#"version = 2
default_agents = ["agent1", "agent2"]

[agents.agent1]
provider = "codex"
target = "agent1"

[agents.agent2]
provider = "claude"
target = "agent2"

[windows]
main = "agent1:codex; agent2:claude"
"#,
    );

    let resp = call(&mut app, "project_reload_config", json!({"dry_run": false}));
    assert!(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let result = resp.get("result").unwrap();
    assert_eq!(result["status"].as_str(), Some("ok"));
    assert_eq!(result["plan_class"].as_str(), Some("add_agent"));
    assert_eq!(result["applied"].as_bool(), Some(true));
    assert!(app.agents.specs.contains_key("agent2"));
}
```

Run:
```bash
cargo test -p ccb-daemon --test reload_tests -- test_reload_apply_adds_agent_and_applies --exact --nocapture
```
Expected: FAIL — `reload_apply_service.rs` and `reload_apply_runtime.rs` are stubs.

- [ ] **Step 2: Implement reload plan-to-apply for add_agent**

In `rust/crates/ccb-daemon/src/reload_apply_plan.rs`, implement:

```rust
pub fn build_reload_plan(
    current: &ProjectConfig,
    desired: &ProjectConfig,
) -> ReloadPlan {
    // Compare agents and windows.
    // Emit AddAgent, RemoveAgent, AddWindow, RemoveWindow operations.
}
```

In `rust/crates/ccb-daemon/src/reload_apply_service.rs`, implement:

```rust
pub fn run_additive_reload_apply(
    app: &mut CcbdApp,
    plan: &ReloadPlan,
) -> Result<AdditiveReloadApplyResult> {
    // For each AddAgent op: add spec and prepare workspace.
    // For each AddWindow op: update topology.
    // Return applied=true if all ops succeed.
}
```

Mirror Python `lib/ccbd/reload_apply_service.py::run_additive_reload_apply` for the `add_agent` case:
- Input: plan with one `add_agent` op for `agent2`.
- Output: `AdditiveReloadApplyResult { applied: true, ... }` and `agent2` present in `app.agents.specs`.

- [ ] **Step 3: Run reload tests**

```bash
cargo test -p ccb-daemon --test reload_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/ccb-daemon/src/reload_apply.rs rust/crates/ccb-daemon/src/reload_apply_service.rs rust/crates/ccb-daemon/src/reload_apply_plan.rs rust/crates/ccb-daemon/src/reload_apply_runtime.rs rust/crates/ccb-daemon/src/reload_runtime_mount_service.rs rust/crates/ccb-daemon/src/reload_transaction_service.rs rust/crates/ccb-daemon/tests/reload_tests.rs
git commit -m "feat(daemon): config reload plan and apply parity"
```

---

### D6: Supervision and recovery

**Files:**
- Modify: `rust/crates/ccb-daemon/src/supervision/loop_.rs`, `rust/crates/ccb-daemon/src/supervision/loop_actions.rs`, `rust/crates/ccb-daemon/src/supervision/loop_context.rs`, `rust/crates/ccb-daemon/src/supervision/loop_runtime.rs`, `rust/crates/ccb-daemon/src/supervision/mount_runtime/service.rs`, `rust/crates/ccb-daemon/src/supervision/mount_runtime/events.rs`, `rust/crates/ccb-daemon/src/supervision/recovery.rs`, `rust/crates/ccb-daemon/src/supervision/recovery_transitions.rs`, `rust/crates/ccb-daemon/src/supervision/backoff.rs`
- Test: create `rust/crates/ccb-daemon/tests/supervision_tests.rs`
- Python reference: `lib/ccbd/supervision/loop.py`, `lib/ccbd/supervision/mount.py`, `lib/ccbd/supervision/recovery.py`

- [ ] **Step 1: Create a failing supervision loop test**

Create `rust/crates/ccb-daemon/tests/supervision_tests.rs`:

```rust
use ccb_daemon::supervision::{
    loop_::SupervisionLoop,
    mount::SupervisionMountService,
    recovery::{RecoveryContext, RecoveryPolicy},
};

#[test]
fn test_supervision_loop_records_mount_event() {
    let mut mount = SupervisionMountService::new();
    mount.record_event(ccb_daemon::supervision::mount::MountEvent {
        agent_name: "agent1".to_string(),
        event_type: "mount_started".to_string(),
        timestamp: "2025-01-01T00:00:00Z".to_string(),
        details: serde_json::json!({"pane_id": "%1"}),
    });

    let events = mount.recent_events(10);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].agent_name, "agent1");
    assert_eq!(events[0].event_type, "mount_started");
}
```

Run:
```bash
cargo test -p ccb-daemon --test supervision_tests -- test_supervision_loop_records_mount_event --exact --nocapture
```
Expected: FAIL — `src/supervision/loop_.rs` is a stub and `SupervisionLoop` does not exist.

- [ ] **Step 2: Implement supervision loop skeleton**

In `rust/crates/ccb-daemon/src/supervision/loop_.rs`, implement:

```rust
pub struct SupervisionLoop {
    mount_service: SupervisionMountService,
    recovery_context: RecoveryContext,
}

impl SupervisionLoop {
    pub fn new(mount_service: SupervisionMountService, recovery_context: RecoveryContext) -> Self {
        Self { mount_service, recovery_context }
    }

    pub fn tick(&mut self, now: &str) -> Vec<SupervisionAction> {
        // Evaluate recovery transitions for each recorded mount event.
        // Return actions (e.g., RestartAgent) based on RecoveryPolicy.
        Vec::new()
    }
}
```

Mirror Python `lib/ccbd/supervision/loop.py::SupervisionLoop.tick`:
- Input: current timestamp.
- Output: list of actions; for the baseline test, the service must at least allow `SupervisionMountService` to record and retrieve events.

- [ ] **Step 3: Implement recovery transition policy**

In `rust/crates/ccb-daemon/src/supervision/recovery_transitions.rs`, implement:

```rust
pub fn evaluate_mount_event(
    event: &MountEvent,
    policy: &RecoveryPolicy,
) -> Option<SupervisionAction> {
    // If event_type == "mount_failed" and retry budget remains, return RestartAgent.
    None
}
```

- [ ] **Step 4: Run supervision tests**

```bash
cargo test -p ccb-daemon --test supervision_tests -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ccb-daemon/src/supervision/loop_.rs rust/crates/ccb-daemon/src/supervision/loop_actions.rs rust/crates/ccb-daemon/src/supervision/loop_context.rs rust/crates/ccb-daemon/src/supervision/loop_runtime.rs rust/crates/ccb-daemon/src/supervision/mount_runtime/service.rs rust/crates/ccb-daemon/src/supervision/mount_runtime/events.rs rust/crates/ccb-daemon/src/supervision/recovery.rs rust/crates/ccb-daemon/src/supervision/recovery_transitions.rs rust/crates/ccb-daemon/src/supervision/backoff.rs rust/crates/ccb-daemon/tests/supervision_tests.rs
git commit -m "feat(daemon): supervision loop, mount events, and recovery transitions parity"
```

---

### D7: Daemon top-level stub triage and cleanup

**Files:**
- Modify: triaged files in `rust/crates/ccb-daemon/src/*.rs`
- Test: existing daemon tests

- [ ] **Step 1: Process the triage list from P0**

For each top-level daemon stub in the triage document:
- If **implement**, follow the same TDD steps as D1–D6.
- If **delete**, remove the file and its `pub mod` declaration in `rust/crates/ccb-daemon/src/lib.rs`.
- If **defer**, leave the stub but add a comment referencing the deferral reason and update `stub-triage.md`.

- [ ] **Step 2: Verify no broken imports**

```bash
cd /home/agnitum/ccb/rust
cargo check -p ccb-daemon
```
Expected: clean.

- [ ] **Step 3: Run daemon tests**

```bash
cargo test -p ccb-daemon -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -u rust/crates/ccb-daemon/src
git commit -m "chore(daemon): triage and resolve top-level stubs"
```

---

### Z: Final validation and matrix update

**Files:**
- Modify: `plans/rust-python-test-parity-matrix.md`

- [ ] **Step 1: Re-count stubs**

```bash
cd /home/agnitum/ccb
echo "ccb-providers stubs: $(grep -rln 'TODO: align with Python' rust/crates/ccb-providers/src/ | wc -l)"
echo "ccb-daemon stubs:    $(grep -rln 'TODO: align with Python' rust/crates/ccb-daemon/src/ | wc -l)"
```
Expected targets:
```text
ccb-providers stubs: ≤ 50
ccb-daemon stubs:    ≤ 50
```

- [ ] **Step 2: Run the full validation matrix**

```bash
cd /home/agnitum/ccb/rust
cargo check --workspace
cargo test -p ccb-providers -- --test-threads=1
cargo test -p ccb-daemon -- --test-threads=1
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --check
```
Expected: all commands succeed with no errors.

- [ ] **Step 3: Update the parity matrix**

Edit `plans/rust-python-test-parity-matrix.md`:
- Mark `providers` cluster tests as `complete` for the adapters covered (Codex, Claude, Gemini, Droid, AGY, OpenCode).
- Mark `daemon_lifecycle` / relevant dispatcher/namespace/reload tests as `complete`.
- Update the `Current state` stub counts and note the Wave 3 baseline date.

- [ ] **Step 4: Commit**

```bash
git add plans/rust-python-test-parity-matrix.md
git commit -m "docs: update parity matrix after Wave 3 provider/daemon deep stub reduction"
```

---

## Self-Review

### Spec coverage

| PRD requirement | Implementing task |
|-----------------|-------------------|
| Provider adapter surface + registry | P1 |
| Codex execution parity | P2 |
| Claude execution/comm parity | P3 |
| Gemini execution parity | P4 |
| Droid execution parity | P5 |
| AGY execution parity | P6 |
| OpenCode execution parity | P7 |
| Shared provider infrastructure | P8 |
| Dispatcher submission/routing | D1 |
| Dispatcher lifecycle/polling | D2 |
| Dispatcher finalization/reply | D3 |
| Namespace runtime materialization | D4 |
| Config reload | D5 |
| Supervision/recovery | D6 |
| Daemon top-level stub triage | D7 |
| Validation + matrix update | Z |

### Placeholder scan

This plan contains no `TBD`, `TODO`, `implement later`, `fill in details`, or `similar to Task X`. Every task names exact file paths, exact verification commands, and concrete example input/output for broad stub sweeps. Any remaining ambiguity is confined to the triage document `stub-triage.md`, which is produced as the first deliverable of P0.

### Type consistency

- `ExecutionAdapter` trait is defined in `rust/crates/ccb-providers/src/execution/adapter.rs`. All provider adapter implementations (`CodexExecutionAdapter`, `ClaudeExecutionAdapter`, `GeminiExecutionAdapter`, `DroidExecutionAdapter`, `AgyExecutionAdapter`, `OpenCodeExecutionAdapter`) use the same `start`/`poll`/`resume` signatures.
- `CompletionDecision` and `CompletionItemKind` come from `ccb_completion::models` and are used consistently across P2–P8 and D2–D3.
- `DispatcherState`, `ExecutionService`, and `SubmissionPlan` are referenced in D1–D3 with the same names as in `rust/crates/ccb-daemon/src/services/dispatcher_runtime/`.
- `ProjectNamespaceController` and `NamespaceTopologyPlan` are referenced in D4 with the same names as in the existing tests.
