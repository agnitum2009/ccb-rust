# Runtime accelerator acceptance evidence — 2026-06-27

Scope: `ccb-legacy` bloodline, Python-compatible runtime with local Rust hotpath replacement.

## What was verified

### Rust accelerator unit contract

Command:

```bash
cargo test --manifest-path rust/Cargo.toml -p ccb-runtime-accelerator -- --test-threads=1
```

Result: pass, 8/8 tests.

Covered tests:
- `capabilities_report_hot_loop_replacement_active`
- `classifies_hot_loop_processes`
- `codex_observe_does_not_emit_assistant_before_anchor`
- `codex_observe_reads_anchor_reply_and_terminal_boundary`
- `codex_observe_reports_missing_session_per_job`
- `default_socket_lives_under_project_ccb`
- `ping_uses_daemon_like_response_shape`
- `unknown_method_fails_loudly`

### Production runtime read-only snapshot

Observed `/home/agnitum/o13` without stopping or changing production CCB.

Relevant process snapshot from `ps -eo pid,ppid,pcpu,pmem,rss,comm,args --sort=-pcpu`:

```text
ccbd/main.py                                ~1.8% CPU, 67,948 KiB RSS
provider_backends.codex.bridge mn_c         ~0.8% CPU, 49,304 KiB RSS
ccbd/keeper_main.py                         ~0.7% CPU, 36,160 KiB RSS
ccb-agent-sidebar instances                 ~0.1% CPU each, ~3.3 MiB RSS each
provider_backends.codex.bridge others       ~0.0% CPU, ~32-34 MiB RSS each
ccb-runtime-accelerator serve               ~0.0% CPU, 4,192 KiB RSS
```

Interpretation:
- The Rust accelerator is present in the live Python CCB runtime: `ccb-runtime-accelerator serve --socket /home/agnitum/o13/.ccb/runtime-accelerator/accelerator.sock`.
- Idle bridge CPU is no longer linearly high across all Codex agents in this snapshot; only `mn_c` showed nonzero bridge CPU (~0.8%).
- `ccbd/main.py` is still the largest Python CCB control-plane CPU consumer in this snapshot, but far below the earlier ~15% observation.

## What was not verified

Python pytest hotpath tests could not run in the current shell because neither `pytest` nor `python3 -m pytest` is installed in the active Python environment:

```text
/root/.local/bin/python: No module named pytest
/root/.local/bin/python3: No module named pytest
```

Skipped command:

```bash
python -m pytest -q \
  test/test_codex_bridge_runtime.py \
  test/test_codex_runtime_accelerator_polling.py \
  test/test_runtime_accelerator_client.py \
  test/test_runtime_accelerator_lifecycle.py
```

## Acceptance status

Current status: partial pass.

Accepted:
- Rust accelerator contract tests pass.
- Live runtime is using the accelerator process.
- Live CPU snapshot shows the previous per-bridge high idle CPU pattern is not present for most Codex bridge processes.

Still pending:
- Run Python pytest hotpath tests in an environment with pytest available.
- Capture before/after benchmark under a controlled idle/active workload, not only a live production snapshot.
- Verify `mn_c` nonzero bridge CPU source if it persists under repeated samples.
