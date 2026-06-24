# Implementation Plan: py2rust CLI management install parity

## Ordered Checklist

1. **Read existing `management_runtime/install.rs`** and `test_cli_management_install.py`.
2. **Add helper functions** to `crates/ccb-cli/src/management_runtime/install.rs`:
   - `resolve_installer_paths`
   - `resolve_managed_install_dir`
   - `build_unix_installer_env`
   - `run_installer`
3. **Add `_detect_git_head(source_dir)`** to read current git commit and date.
4. **Create tests** in `crates/ccb-cli/tests/management_install_tests.rs` mirroring the four Python tests.
5. **Update `plans/rust-python-test-parity-matrix.md`** to note `test_cli_management_install.py` parity.
6. Run `cargo test -p ccb-cli --test management_install_tests -- --test-threads=1`.
7. Run `cargo fmt --all -- --check`.
8. Run `cargo clippy -p ccb-cli --tests`.
9. Commit and archive task.

## Validation Commands

```bash
cargo test -p ccb-cli --test management_install_tests -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-cli --tests
```

## Risky Files / Rollback Points

- `crates/ccb-cli/src/management_runtime/install.rs` — path/env logic; keep deterministic and side-effect-free in unit tests.
- `run_installer` spawns a child process; tests must use a harmless shell script and temp dirs.
