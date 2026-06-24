# Wave 4: 端到端恢复与边缘 parity

## Problem

After Waves 1–3 the Rust workspace has working `Phase2Services`, runtime launch, provider adapters, daemon handlers, and per-cluster parity tests, but several cross-cutting gaps remain:

1. **End-to-end multi-agent session persistence/recovery** is only partially covered. Python `test_v2_ccbd_*.py` exercises keeper state, lifecycle progress, reload handoff, full socket round-trip with dispatcher/completion/mailbox, runtime mount ownership, and supervision loops. Rust has focused reload and namespace tests but lacks the full multi-agent recovery story.
2. **Terminal namespace / pane identity** has unit-level parity (`tmux_runtime_namespace_tests.rs`, `tmux_runtime_state_tests.rs`, `identity.rs` inline tests) but no integration tests that verify the daemon writes pane identity options during start/restore and that namespace state survives a daemon restart.
3. **27 unmatched Python tests** in `plans/rust-python-test-parity-matrix.md` have no Rust cluster mapping. They span install/update metadata, MCP delegation, sidebar click/resize sync, runtime env control plane, active runtime polling, ask/restart CLI edge cases, Windows/WSL helpers, skill templates, repo hygiene, and stability regressions.

The goal of this wave is to close the in-scope gaps, explicitly retire the out-of-scope items, and update the parity matrix and migration roadmap so the migration finish line is unambiguous.

## Scope

### In scope

| Sub-area | Python reference | Rust target |
|---|---|---|
| Multi-agent session persistence/recovery | `test_v2_ccbd_keeper.py`, `test_v2_ccbd_socket.py`, `test_v2_ccbd_start_flow.py`, `test_v2_ccbd_supervision_loop.py`, `test_v2_ccbd_ping_runtime.py`, `test_v2_ccbd_dispatcher.py`, `test_v2_ccbd_mount_ownership.py` | `rust/crates/ccb-daemon/tests/` integration tests; `ccb-daemon/src/reload*.rs`, `ccb-daemon/src/services/project_namespace_runtime/`, `ccb-agents` restore stores |
| Terminal namespace / pane identity | `test_ccbd_tmux_namespace.py`, `test_ccbd_tmux_state.py` | `rust/crates/ccb-daemon/tests/tmux_runtime_*_tests.rs`, `rust/crates/ccb-terminal/src/identity.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/` |
| Install runtime parity (core Rust functions) | `test_install_*.py` identity/line-endings/tar-safety/source-dev/watchdog/droid-delegation subsets | `rust/crates/ccb-cli/src/management_runtime/install.rs`, `rust/crates/ccb-cli/tests/management_install_tests.rs` |
| MCP delegation parity | `test_mcp_delegation_server.py`, `test_mcp_delegation_server_runtime_tools.py` | `rust/tools/ccb-mcp-server/src/lib.rs`, `rust/tools/ccb-mcp-server/tests/` |
| Sidebar click/resize sync parity | `test_sidebar_click.py`, `test_sidebar_resize_sync.py` | `rust/crates/ccb-cli/src/sidebar_click.rs`, `rust/crates/ccb-cli/src/sidebar_resize_sync.rs`, `rust/crates/ccb-cli/tests/sidebar_*_tests.rs` |
| Active runtime polling parity | `test_active_runtime_polling.py` | `rust/crates/ccb-completion/` and/or `rust/crates/ccb-providers/` adapter layer |
| Ask/restart CLI edge parity | `test_ask_cli.py`, `test_ask_internal_paths.py`, `test_ccb_restart.py` | `rust/crates/ccb-cli/src/entry.rs`, `rust/crates/ccb-cli/src/commands.rs`, `rust/crates/ccb-cli/src/phase2_runtime/`, `rust/crates/ccb-cli/tests/` |
| Runtime env control plane parity | `test_runtime_env_control_plane.py` | `rust/crates/ccb-runtime-env/src/control_plane.rs` |
| Stability regressions | `test_stability_regressions.py` | Provider log-reader / execution adapter tests in `rust/crates/ccb-providers/tests/` |
| Matrix/roadmap updates | `plans/rust-python-test-parity-matrix.md`, `.trellis/spec/migration-roadmap.md` | Decision records for out-of-scope tests |

### Out of scope (recorded explicitly)

| Test / area | Rationale | Where recorded |
|---|---|---|
| `test_install_identity_output.py` | Tests `install.sh` bash functions `print_install_identity_summary`/`write_install_metadata`; the release install flow is driven by `install.sh` which remains a bash script. Core Rust install functions (`resolve_installer_paths`, `build_unix_installer_env`, `run_installer`) are in scope. | Parity matrix + migration-roadmap |
| `test_install_major_upgrade_guard.py` | Tests `install.sh` bash function `require_major_upgrade_confirmation`. | Parity matrix + migration-roadmap |
| `test_install_root_confirmation.py` | Tests `install.sh` bash root-confirm flow. | Parity matrix + migration-roadmap |
| `test_install_script_sidebar.py` | Tests `install.sh` / `bin/build-ccb-agent-sidebar` / packaging scripts; sidebar TUI is a native Rust binary, but install-time build/packaging remains bash. | Parity matrix + migration-roadmap |
| `test_install_source_dev_mode.py` | Tests source-dev wrapper provisioning inside `install.sh`; native Rust release artifacts replace wrapper scripts. | Parity matrix + migration-roadmap |
| `test_install_watchdog_optional.py` | Tests `install.sh` optional watchdog/toml/role-pack/neovim provisioning. | Parity matrix + migration-roadmap |
| `test_install_droid_delegation.py` | Tests `install.sh` Droid MCP delegation registration via the `droid` CLI. | Parity matrix + migration-roadmap |
| `test_windows_bootstrap_script.py` | Tests `scripts/bootstrap-windows-test-env.ps1`; no Rust Windows bootstrap exists and the team has not committed to a Windows parity target. | Parity matrix + migration-roadmap |
| `test_wsl_path_utils.py` | Tests `terminal._extract_wsl_path_from_unc_like_path`; WSL path helpers are intentionally not ported per `migration-roadmap.md`. | Parity matrix + migration-roadmap |
| `test_ask_skill_templates.py` | Tests skill markdown text in `inherit_skills/`; these are human-facing instruction files, not runtime code. | Parity matrix + migration-roadmap |
| `test_ccb_github_skill.py` | Tests `dev_tools/skills/ccb-github/scripts/check_release_state.py`; this is a release-management skill, not CCB runtime. | Parity matrix + migration-roadmap |
| `test_repo_hygiene.py` | Tests repository layout/skill hygiene; not runtime behavior. | Parity matrix + migration-roadmap |

## Acceptance Criteria

- [ ] Every in-scope sub-area has at least one Rust integration test that mirrors the Python reference assertion set.
- [ ] `rust/crates/ccb-daemon/tests/` covers multi-agent recovery: keeper state roundtrip, lifecycle progress fields, reload handoff signature mismatch, socket round-trip with submit/ask/delivery, runtime mount ownership, and supervision loop recovery.
- [ ] `rust/crates/ccb-terminal/src/identity.rs` and `ccb-daemon` namespace integration tests verify pane identity options (`@ccb_*`) are written during start/restore and survive namespace reload.
- [ ] `rust/crates/ccb-cli/src/management_runtime/install.rs` gains `safe_extract_tar` and line-ending normalization tests; existing `management_install_tests.rs` passes.
- [ ] `rust/tools/ccb-mcp-server/tests/` exists and passes, covering tool definitions and ask/pend/ping handlers.
- [ ] `rust/crates/ccb-cli/src/sidebar_click.rs` and `sidebar_resize_sync.rs` are implemented with tests matching Python `test_sidebar_click.py` and `test_sidebar_resize_sync.py`.
- [ ] `rust/crates/ccb-runtime-env/src/control_plane.rs` inline tests already cover `test_runtime_env_control_plane.py`; confirm with a dedicated smoke run and update matrix to `complete`.
- [ ] `test_active_runtime_polling.py` behavior is mapped to Rust completion/provider polling tests.
- [ ] `test_ask_cli.py`, `test_ask_internal_paths.py`, and `test_ccb_restart.py` edge cases are covered in Rust CLI tests.
- [ ] `plans/rust-python-test-parity-matrix.md` is updated: in-scope clusters move toward `complete`, out-of-scope items are marked with rationale.
- [ ] `.trellis/spec/migration-roadmap.md` Out-of-scope section is updated with the 12 retired tests and rationale.
- [ ] `cargo test --workspace -- --test-threads=1` passes.
- [ ] `cargo clippy --workspace --all-targets` reports 0 errors.
- [ ] `cargo fmt --check` is clean.

## References

- Parent design: `.trellis/tasks/06-24-py2rust-remaining-parity/design.md`
- Migration roadmap: `.trellis/spec/migration-roadmap.md`
- Parity matrix: `plans/rust-python-test-parity-matrix.md`
- Python reference tests: `test/test_v2_ccbd_*.py`, `test/test_ccbd_tmux_namespace.py`, `test/test_ccbd_tmux_state.py`, `test/test_install_*.py`, `test/test_mcp_delegation_server*.py`, `test/test_sidebar_*.py`, `test/test_windows_bootstrap_script.py`, `test/test_wsl_path_utils.py`, `test/test_runtime_env_control_plane.py`, `test/test_active_runtime_polling.py`, `test/test_ask_cli.py`, `test/test_ask_internal_paths.py`, `test/test_ccb_restart.py`, `test/test_stability_regressions.py`
- Rust targets:
  - `rust/crates/ccb-daemon/src/reload*.rs`
  - `rust/crates/ccb-daemon/src/services/project_namespace_runtime/`
  - `rust/crates/ccb-terminal/src/identity.rs`
  - `rust/crates/ccb-cli/src/management_runtime/install.rs`
  - `rust/tools/ccb-mcp-server/src/lib.rs`
  - `rust/crates/ccb-cli/src/sidebar_click.rs`
  - `rust/crates/ccb-cli/src/sidebar_resize_sync.rs`
  - `rust/crates/ccb-runtime-env/src/control_plane.rs`
