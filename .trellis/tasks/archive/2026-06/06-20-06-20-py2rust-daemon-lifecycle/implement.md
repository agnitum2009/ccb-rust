# Daemon lifecycle parity — implementation plan

## Phase A — runtime_launch provider launchers

目标：覆盖 `test_v2_runtime_launch.py` 中 provider-specific `build_start_cmd` 测试。

1. **Codex launcher**
   - 在 `rust/crates/ccb-providers/src/providers/codex.rs` 中新增/暴露 `build_start_cmd`。
   - 新建 `rust/crates/ccb-providers/src/codex/launcher_runtime/` 模块（`home.rs`、`command.rs`、`session_paths.rs`），对齐 Python `lib/provider_backends/codex/launcher_runtime/`。
   - 实现 `prepare_codex_home_overrides`、`resolve_codex_home_layout`、`build_codex_shell_prefix`、`load_resume_session_id`。
   - 添加 `crates/ccb-providers/tests/codex_launcher_tests.rs`，从 `test_v2_runtime_launch.py` 选取等价断言点。

2. **Claude launcher**
   - 填充 `rust/crates/ccb-providers/src/claude/launcher.rs`。
   - 完善 `claude/launcher_runtime/home.rs`（已存在，需补 API env / managed home / settings overlay / root sandbox / auth projection 逻辑）。
   - 添加 `crates/ccb-providers/tests/claude_launcher_tests.rs`。

3. **Gemini / AGY / Droid launchers**
   - 对应 Rust launcher 已存在但缺少 parity 测试；按 Python 补齐 `build_start_cmd` 细节（profile API env、session transport env）。
   - 添加 `gemini_launcher_tests.rs`、`agy_launcher_tests.rs`、`droid_launcher_tests.rs`。

4. **OpenCode launcher**
   - 已部分实现；补 `build_session_payload` / `post_launch` 以及完整 `ensure_agent_runtime` 集成测试。

## Phase B — runtime_launch orchestration

目标：覆盖 `test_v2_runtime_launch.py` 中 `ensure_agent_runtime` 编排测试。

1. 实现 `ccb-daemon/src/start_runtime/agent_runtime_binding.rs` 的 `resolve_runtime_binding_state`（复用 `ccb-provider-core::session_binding` 策略）。
2. 实现 `ccb-daemon/src/start_runtime/agent_runtime.rs` 的 `start_agent_runtime` 和测试友好的 `ensure_agent_runtime` 包装。
3. 扩展 `ccb-daemon/src/provider_launcher.rs` 的 `build_launch_plan`，支持 `codex`/`claude`/`gemini`/`agy`/`droid` 分支。
4. 添加 `ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`。

## Phase C — ps / wait / start CLI services

目标：覆盖 `test_v2_ps_service.py`、`test_v2_wait_service.py`、`test_v2_start_service.py`、`test_v2_start_foreground.py`、`test_v2_daemon_startup_wait.py`。

1. 在 `ccb-cli/src/services/` 下实现/补全 `ps.rs`、`wait.rs`、`start.rs`。
2. 复用 `ccb-cli/src/services/daemon_client.rs` 的 socket client。
3. 添加 `ccb-cli/tests/cli_ps_tests.rs`、`cli_wait_tests.rs`、`cli_start_tests.rs`。

## Validation commands

- `cargo test -p ccb-providers -- --test-threads=1`
- `cargo test -p ccb-daemon -- --test-threads=1`
- `cargo test -p ccb-cli -- --test-threads=1`
- `cargo clippy -p ccb-providers --tests -- -D warnings`
- `cargo clippy -p ccb-daemon --tests -- -D warnings`
- `cargo clippy -p ccb-cli --tests -- -D warnings`

## Review gates

- 每个 provider launcher 的 Rust 测试至少覆盖 Python 对应测试的 3 个核心断言点。
- 新增代码必须通过 `cargo fmt` 和 `cargo clippy`。
- 更新 parity matrix 后再调用 `task.py archive`。
