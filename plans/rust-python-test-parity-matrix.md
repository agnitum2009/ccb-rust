# Rust/Python Test Parity Matrix

Generated during Phase 5 of the Rust migration alignment (v7.5.2).

## Summary

- Python reference tests (v7.5.2): **314**
- Rust migration tests: **60**
- Coverage: heuristic groupings below; many Python tests cover provider/runtime behavior that is now exercised by focused Rust integration tests.
- Intentionally out of scope: provider-specific UI/CLI wrappers that are replaced by native Rust binaries (`ask`, `autonew`, `ctx-transfer`, `ccb` itself).

## Cluster Mapping

| Area | Python Tests | Rust Tests | Status | Notes |
|------|--------------|------------|--------|-------|
| release_build | `test_build_linux_release_script.py`, `test_build_macos_release_script.py` | `tools/ccb-release-builder/src/lib.rs` | partial | Release scripts now delegate to `ccb-release-builder`; validation test added. |
| cli_entrypoint | `test_cli_daemon_keeper_runtime.py`, `test_cli_kill_runtime_processes.py`, `test_cli_kill_runtime_zombies.py`, `test_cli_management_install.py`, `test_cli_management_update.py` (+20 more) | `crates/ccb-cli/tests/cli_integration_tests.rs`, `crates/ccb-cli/tests/cli_stub_commands_tests.rs`, `crates/ccb-cli/tests/cli_maintenance_tests.rs`, `crates/ccb-cli/tests/helper_binaries_tests.rs` | partial | Rust CLI tests cover start/ask/kill/status; `ask`/`autonew`/`ctx-transfer` helper binaries now match Python `--help` behavior (py2rust-cli). |
| daemon_lifecycle | `test_ccbd_client_resolution.py`, `test_ccbd_comms_recover.py`, `test_ccbd_health_assessment_provider_pane.py`, `test_ccbd_health_monitor_rebind.py`, `test_ccbd_namespace_additive_patch.py` (+41 more) | `crates/ccb-daemon/tests/daemon_integration_tests.rs`, `crates/ccb-daemon/tests/handler_fault_tests.rs`, `crates/ccb-daemon/tests/handler_maintenance_tests.rs` (+1 more) | partial | Daemon handlers, start/stop flows, and health assessment are covered in Rust (py2rust-daemon). Remaining provider-specific lifecycle tests deferred to py2rust-providers. |
| providers | `test_agy_execution_polling.py`, `test_claude_assistant_events.py`, `test_claude_binding_runtime_session.py`, `test_claude_comm.py`, `test_claude_comm_binding.py`, `test_v2_provider_core_registry.py`, `test_v2_provider_health_store.py`, `test_v2_provider_restore_launchers.py` (+88 more) | `crates/ccb-providers/tests/provider_codebuddy_tests.rs`, `crates/ccb-providers/tests/execution_tests.rs`, `crates/ccb-providers/tests/provider_gemini_tests.rs`, `crates/ccb-providers/tests/runtime_tests.rs`, `crates/ccb-providers/tests/provider_session_paths_tests.rs`, `crates/ccb-provider-core/tests/source_home_tests.rs`, `crates/ccb-provider-core/tests/registry_tests.rs` (+15 more) | partial | Each major provider has a Rust integration test file; provider-core `source_home` passwd fallback parity added (py2rust-providers); provider-core registry default session-binding and runtime-launcher maps parity added (py2rust-providers-deep); provider health snapshot store job-history parity added (py2rust-providers-health-store); provider-agnostic session path helpers with relocated runtime anchor parity added (py2rust-providers-restore-launchers); `resolve_codex_home_layout` parity with integration tests added (py2rust-provider-profiles); `prepare_provider_workspace` orchestrator and provider home materializers in progress. |
| mailbox | `test_message_bureau_control_queue.py`, `test_message_bureau_submission_fastpath.py`, `test_v2_mailbox_kernel_service.py`, `test_v2_mailbox_kernel_store.py`, `test_v2_message_bureau_dispatcher_integration.py` (+1 more) | `crates/ccb-mailbox/tests/integration.rs`, `crates/ccb-message-bureau/tests/smoke.rs` | partial | Mailbox kernel and message-bureau facade/control/trace are complete (py2rust-mailbox). Remaining dispatcher/fastpath integration parity deferred to py2rust-daemon and py2rust-parity. |
| terminal_runtime | `test_agents_layout_runtime.py`, `test_ccbd_start_runtime_layout.py`, `test_ccbd_tmux_namespace.py`, `test_ccbd_tmux_state.py`, `test_terminal_runtime_backend_env.py` (+19 more) | `crates/ccb-terminal/tests/test_backend.rs`, `crates/ccb-terminal/tests/test_layouts.rs`, `crates/ccb-terminal/tests/test_pane_service.rs` (+3 more) | partial | Tmux backend, layouts, and pane registry tests pass (py2rust-terminal). Remaining namespace/state integration parity deferred to py2rust-daemon. |
| storage_paths | `test_claude_binding_runtime_session.py`, `test_claude_session_auto_transfer.py`, `test_claude_session_fields.py`, `test_claude_session_index_runtime.py`, `test_claude_session_pathing.py` (+37 more) | `crates/ccb-storage/tests/integration_storage_classification.rs`, `crates/ccb-storage/tests/integration_storage_paths.rs`, `crates/ccb-storage/tests/integration_text_artifacts.rs` (+1 more) | partial | Storage paths, classification, and text artifacts in Rust are complete (py2rust-core). Provider session pathing tests remain in Python reference; covered by py2rust-providers. |
| agents_roles | `test_agents_layout_runtime.py`, `test_role_lock_refresh.py`, `test_rolepacks.py` | `crates/ccb-agents/tests/rolepack_tests.rs` | partial | Role packs and role lock refresh (`find_project_role_lock_updates`, `confirm_project_role_lock_refresh`) covered (py2rust-agents). Broader agent workspace tests still in Python. |
| completion | `test_agy_execution_polling.py`, `test_claude_execution_polling.py`, `test_claude_execution_runtime_start.py`, `test_codex_execution_polling.py`, `test_droid_execution_polling.py` (+17 more) | `crates/ccb-completion/tests/integration_tests.rs`, `crates/ccb-jobs/tests/store_integration.rs` | partial | Job store Rust tests updated for Python-compatible `JobEvent.type` field and `schema_version:2`/`record_type` JSONL headers (py2rust-jobs, py2rust-parity). Full completion orchestration parity deferred to py2rust-completion. |
| heartbeat | `test_maintenance_heartbeat.py`, `test_v2_heartbeat_engine.py` | `crates/ccb-heartbeat/tests/integration.rs` | partial | Heartbeat engine and maintenance heartbeat models covered in Rust (py2rust-jobs). Maintenance classifier parity remains in Python reference. |
| memory | `test_memory_auto_transfer.py`, `test_memory_module.py`, `test_memory_transfer_providers.py`, `test_memory_transfer_session_binding.py`, `test_project_memory.py` (+5 more) | `crates/ccb-memory/tests/integration_tests.rs` | partial | Memory integration covered; workspace-binding session discovery parity added (py2rust-memory). |
| config_project | `test_ccbd_project_clear.py`, `test_ccbd_project_focus.py`, `test_ccbd_project_view.py`, `test_gemini_project_hash_candidates.py`, `test_project_id.py` (+12 more) | `crates/ccb-project/tests/smoke.rs`, `crates/ccb-workspace/tests/smoke.rs` | partial | Core project/workspace discovery, identity, resolver, and binding are complete (py2rust-project). Remaining daemon project commands and provider hash candidates tests are covered by py2rust-daemon and py2rust-providers. |
| types_i18n | `test_ccb_protocol.py`, `test_claude_protocol.py`, `test_codebuddy_protocol.py`, `test_copilot_protocol.py`, `test_droid_protocol.py` (+2 more) | `crates/ccb-types/tests/control_plane.rs`, `crates/ccb-types/tests/env.rs`, `crates/ccb-types/tests/i18n.rs` (+1 more) | partial | Core types, env, i18n, and control-plane contracts in Rust are complete (py2rust-core). Provider protocol tests remain in Python reference; covered by py2rust-providers. |

## Python Tests Not Matched to a Cluster

- `test_active_runtime_polling.py`
- `test_ask_cli.py`
- `test_ask_internal_paths.py`
- `test_ask_skill_templates.py`
- `test_ccb_github_skill.py`
- `test_ccb_restart.py`
- `test_cleanup_service.py`
- `test_compat_stdin_decode.py`
- `test_detect_terminal.py`
- `test_doctor_runtime_identity.py`
- `test_ensure_pane_stale.py`
- `test_env_utils.py`
- `test_install_identity_output.py`
- `test_install_line_endings.py`
- `test_install_major_upgrade_guard.py`
- `test_install_root_confirmation.py`
- `test_install_script_sidebar.py`
- `test_install_source_dev_mode.py`
- `test_install_tar_safety.py`
- `test_install_watchdog_optional.py`
- `test_management_cleanup.py`
- `test_mcp_delegation_server.py`
- `test_mcp_delegation_server_runtime_tools.py`
- `test_multi_instance.py`
- `test_pane_log_communicator_state.py`
- `test_pane_log_support_parsing.py`
- `test_pane_quiet_support.py`
- `test_provider_activity_artifacts.py`
- `test_provider_activity_hook_script.py`
- `test_provider_finish_hook_script.py`
- `test_provider_helper_cleanup.py`
- `test_provider_hook_settings.py`
- `test_provider_hook_transcript.py`
- `test_provider_instance_resolution.py`
- `test_provider_profiles.py`
- `test_provider_source_home.py`
- `test_registry_cleanup.py`
- `test_registry_lookup.py`
- `test_reply_delivery_formatting.py`
- `test_repo_hygiene.py`
- `test_runtime_env_control_plane.py`
- `test_runtime_specs.py`
- `test_sidebar_click.py`
- `test_sidebar_resize_sync.py`
- `test_source_runtime_guard.py`
- `test_stability_regressions.py`
- `test_v2_agent_store.py`
- `test_v2_api_models.py`
- `test_v2_ask_service.py`
- `test_v2_daemon_startup_wait.py`
- `test_v2_diagnostics_bundle.py`
- `test_v2_fault_injection.py`
- `test_v2_kill_service.py`
- `test_v2_policy.py`
- `test_v2_provider_binding.py`
- `test_v2_provider_catalog.py`
- `test_v2_ps_service.py`
- `test_v2_runtime_isolation.py`
- `test_v2_runtime_launch.py`
- `test_v2_start_foreground.py`
- `test_v2_start_service.py`
- `test_v2_wait_service.py`
- `test_windows_bootstrap_script.py`
- `test_wsl_path_utils.py`

## Rust Tests Not Matched to a Cluster

- `crates/ccb-provider-hooks/tests/provider_hooks_integration.rs`
- `crates/ccb-provider-profiles/tests/integration_provider_profiles.rs`
- `crates/ccb-provider-sessions/tests/integration.rs`
- `crates/ccb-runtime-env/tests/smoke.rs`
- `crates/ccb-runtime-pid-cleanup/tests/smoke.rs`
- `crates/ccb-stdio-runtime/tests/smoke.rs`
- `crates/ccb-ui-text/tests/smoke.rs`
- `tools/ccb-release-builder/tests/integration_tests.rs`
- `tools/ccb-release-checker/tests/integration_tests.rs`

## Coverage Gaps & Out-of-Scope Items

### Gaps
- End-to-end multi-agent session persistence and recovery: partially covered by daemon reload tests, but no full parity with Python `test_v2_ccbd_*` suite yet.
- Real provider CLI integration (Codex, Claude, Gemini, etc.): intentionally mocked in Rust; live CLI tests remain in Python reference.
- Windows bootstrap and WSL path utilities: no Rust equivalents.
- `install.sh` itself is bash; coverage relies on the new `test_rust_release_artifact.py` validation test.

### Intentionally Out of Scope
- Python wrapper scripts (`bin/ask`, `bin/autonew`, `bin/ctx-transfer`, `ccb`) are replaced by native Rust binaries in release artifacts.
- `lib/` Python implementation is excluded from release tarballs; runtime behavior is provided by Rust crates.
- Provider-specific Python hook scripts (e.g., `bin/ccb-provider-activity-hook`) are retained for source installs but not required in release artifacts.

## How to Use This Matrix

1. When retiring a Python test, verify the mapped Rust test covers the same behavior.
2. When adding Rust tests, update this matrix with the new mapping.
3. Treat clusters marked `partial` as candidates for deeper parity work in follow-up phases.
