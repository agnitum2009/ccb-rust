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

### Python hotpath compatibility tests

The plain active Python environment lacks pytest, so the runnable project path is `uv run --with pytest`.

Command:

```bash
uv run --with pytest pytest -q \
  test/test_codex_bridge_runtime.py \
  test/test_codex_runtime_accelerator_polling.py \
  test/test_runtime_accelerator_client.py \
  test/test_runtime_accelerator_lifecycle.py
```

Result: pass, `17 passed in 0.71s`.

## What was not verified


## Acceptance status

Current status: partial pass.

Accepted:
- Rust accelerator contract tests pass.
- Python hotpath compatibility tests pass via `uv run --with pytest`.
- Live runtime is using the accelerator process.
- Live CPU snapshot shows the previous per-bridge high idle CPU pattern is not present for most Codex bridge processes.

Still pending:
- Capture before/after benchmark under a controlled idle/active workload, not only a live production snapshot.
- Verify `mn_c` nonzero bridge CPU source if it persists under repeated samples.

## 2026-06-27 o13 read-only CPU resample

Files:
- `.benchmarks/2026-06-27-o13-cpu-resample.tsv`
- `.benchmarks/2026-06-27-o13-cpu-resample-summary.txt`

Method: five read-only `ps` samples, five seconds apart, exact-match filtered to `/root/.local/share/codex-dual` production CCB processes for `/home/agnitum/o13`.

Summary:

```text
accelerator                 avg=0.000 max=0.000 n=5
ccbd-keeper                 avg=0.700 max=0.700 n=5
ccbd-main                   avg=1.800 max=1.800 n=5
codex-bridge archi          avg=0.000 max=0.000 n=5
codex-bridge ccb_self       avg=0.000 max=0.000 n=5
codex-bridge coder          avg=0.000 max=0.000 n=5
codex-bridge mn_c           avg=0.800 max=0.800 n=5
codex-bridge mother         avg=0.000 max=0.000 n=5
sidebar                     avg=0.100 max=0.100 n=25
```

Interpretation:
- The earlier all-bridge hot-loop pattern is not present in this resample.
- `mn_c` remains the only bridge with stable nonzero CPU in this window; it needs targeted source attribution if it matters operationally.
- This is still a production read-only snapshot, not a controlled before/after benchmark.
