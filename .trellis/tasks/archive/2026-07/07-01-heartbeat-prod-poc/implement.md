# Implementation Plan: Heartbeat PyO3 Production PoC

## Step 1: Build release `.so`

```bash
cd /home/agnitum/ccb/ccb-legacy/rust
cargo build --release -p ccb-py-heartbeat
```

Output: `target/release/libccb_py_heartbeat.so`

## Step 2: Stage into production Python path

Determine the Python `site-packages` or `PYTHONPATH` entry used by the running
`ccbd` process. Then install the extension with the correct ABI suffix:

```bash
EXT_SUFFIX=$(python3 -c 'import sysconfig; print(sysconfig.get_config_var("EXT_SUFFIX"))')
DST_DIR=/root/.local/share/codex-dual/lib   # adjust if ccbd uses a different path
install -m 755 target/release/libccb_py_heartbeat.so \
  "$DST_DIR/ccb_py_heartbeat$EXT_SUFFIX"
```

Verify import from the same Python interpreter `ccbd` uses:

```bash
PYTHONPATH="$DST_DIR" python3 -c "import ccb_py_heartbeat; print(ccb_py_heartbeat.SCHEMA_VERSION)"
```

## Step 3: Create Python shim

Create a file that shadows `heartbeat` for `ccbd`:

```text
/root/.local/share/codex-dual/lib/heartbeat.py
```

Content:

- If `os.environ.get('CCB_HEARTBEAT_RUST') in ('1', 'true', 'yes')`:
  import from `ccb_py_heartbeat` and wrap results in dataclass-like objects.
- Else: re-export from the real `/home/agnitum/ccb-git/lib/heartbeat/`.

This lets us toggle without editing files while `ccbd` runs.

## Step 4: Verify shadowing

From the `ccbd` Python interpreter:

```python
import heartbeat
print(heartbeat.__file__)  # should point to the shim
```

## Step 5: Enable Rust path

Set the env var in the `ccbd` environment. If `ccbd` is launched by a wrapper,
edit the wrapper or the systemd/tmux launch command to export
`CCB_HEARTBEAT_RUST=1`. For the PoC, we can also patch the shim default to
`True` so no restart is needed.

## Step 6: Observe

Collect baseline before flipping the switch:

```bash
ps -o pid,rss,comm -p <ccbd_pid>
```

Then flip the switch and collect every 60 seconds for 10–30 minutes:

```bash
while true; do
  date
  ps -o pid,rss,comm -p <ccbd_pid>
  # optionally: grep ccbd log for heartbeat errors
  sleep 60
done
```

## Step 7: Test rollback

```bash
# Option A: remove env var and reload shim (if shim supports env gate)
unset CCB_HEARTBEAT_RUST
# Option B: delete shim and let Python clear import cache
rm /root/.local/share/codex-dual/lib/heartbeat.py
```

Verify `import heartbeat` returns to `/home/agnitum/ccb-git/lib/heartbeat/__init__.py`.

## Step 8: Record evidence

Write a short report in the task directory:

- `research/heartbeat-prod-poc-report.md`
- Baseline vs PoC RSS
- Any errors
- Rollback test result
- Recommendation

## Step 9: Await user confirmation

Do not create or update upstream PRs until the user explicitly approves the PoC
results.

## Validation commands

```bash
# Rust side
cargo test -p ccb-py-heartbeat -- --test-threads=1
cargo clippy -p ccb-py-heartbeat -- -D warnings

# Python side
PYTHONPATH=/root/.local/share/codex-dual/lib python3 tests/smoke.py

# Production import check
PYTHONPATH=/root/.local/share/codex-dual/lib python3 - <<'PY'
import heartbeat
print(heartbeat.__file__)
PY
```
