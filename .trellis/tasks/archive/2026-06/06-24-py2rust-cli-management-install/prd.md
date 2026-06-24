# py2rust CLI management install parity

## Goal

Implement the `management_runtime/install.rs` helpers so that the four Python tests in `test/test_cli_management_install.py` have Rust equivalents and pass.

## Requirements

1. Implement `resolve_installer_paths(action, script_root)` returning `(source_root, install_dir)`:
   - If `script_root` is a source repo root (`install.sh` + `.git`), `source_root` is `script_root`.
   - `install_dir` is resolved by `resolve_managed_install_dir(script_root)`.
   - The `action` argument is accepted for API parity but does not change behavior for the `install` action.

2. Implement `resolve_managed_install_dir(script_root)`:
   - If `CODEX_INSTALL_PREFIX` is set and non-empty, use it (with `~` expansion).
   - Otherwise fall back to a default managed install path.
   - For a source repo root, the managed prefix is still returned (mirroring Python).

3. Implement `build_unix_installer_env(install_dir, source_dir)` returning a `HashMap<String, String>`:
   - Set `CODEX_INSTALL_PREFIX` to `install_dir`.
   - Set `CCB_SOURCE_KIND` to `"source"`.
   - Set `CCB_SOURCE_ROOT` to `source_dir`.
   - Set `CCB_GIT_COMMIT` and `CCB_GIT_DATE` from the current git HEAD of `source_dir`.
   - Preserve any existing `CODEX_INSTALL_PREFIX` from the environment? No — explicit `install_dir` wins.

4. Implement `run_installer(action, script_root)` returning an exit code (`i32`):
   - Resolve `source_root` and `install_dir` via `resolve_installer_paths`.
   - Find the installer script (`install.sh` on Unix, `install.ps1` on Windows).
   - Stage the script in a temporary directory with a deterministic `ccb-installer-` prefix.
   - Normalize CRLF line endings to LF before writing the staged copy.
   - Run the staged installer with the environment from `build_unix_installer_env` and `CODEX_INSTALL_PREFIX` set.
   - Return the installer's exit code (or a non-zero code on failure).

## Acceptance Criteria

- [ ] `cargo test -p ccb-cli --test management_install_tests -- --test-threads=1` passes with new tests mirroring the four Python tests.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy -p ccb-cli --tests` produces no new warnings.
- [ ] `plans/rust-python-test-parity-matrix.md` notes that `test_cli_management_install.py` parity is covered.

## Out of Scope

- `update` command / post-update provisioning (large, separate task).
- Windows `install.ps1` execution path (keep cross-platform helpers but only test Unix path).
- Tarball download/extract (not exercised by `test_cli_management_install.py`).
