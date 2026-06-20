# Provider 后端深层 parity — Registry 静态映射阶段

## Goal

补齐 `ccb-provider-core` 中 provider registry 的默认 `session_binding` 与 `runtime_launcher` 静态映射，和 Python `test_v2_provider_core_registry.py` 保持 parity。

## Scope

- `rust/crates/ccb-provider-core/src/registry.rs`
- `rust/crates/ccb-provider-core/tests/registry_tests.rs`（新建）

## Requirements

1. `build_default_session_binding_map(include_optional)` 在 `include_optional=true` 时返回包含 8 个 provider 的映射：
   - `codex`, `claude`, `gemini`, `opencode`, `droid`, `agy`, `kimi`, `deepseek`
   - 每个 `ProviderSessionBinding` 的 `provider`、`session_id_attr`、`session_path_attr` 与 Python 后端默认值一致。
   - `opencode` 的 `session_path_attr` 为 `"session_file"`，其余为 `"<provider>_session_path"`。
2. `build_default_runtime_launcher_map(include_optional)` 在 `include_optional=true` 时返回包含上述 8 个 provider 的映射：
   - `codex` 的 `launch_mode` 为 `LaunchMode::CodexTmux`。
   - 其余 provider 的 `launch_mode` 为 `LaunchMode::SimpleTmux`。
3. `include_optional=false` 时仅返回 core provider（`codex`, `claude`, `gemini`）的映射。
4. 新建 `registry_tests.rs` 覆盖 Python `test_v2_provider_core_registry.py` 中的关键断言：
   - session binding map 的 key set 与代表性 attr 值。
   - runtime launcher map 的 key set 与代表性 launch_mode 值。
   - `session_filename_for_agent` 的 agent-first 命名（已在 `pathing.rs` 有基础覆盖，本测试补充 codex/agy/kimi/deepseek 用例）。

## Acceptance Criteria

- [ ] `cargo test -p ccb-provider-core -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-provider-core -- -D warnings` 通过。
- [ ] `cargo fmt -p ccb-provider-core` 无需产生新 diff。
- [ ] parity matrix 已更新 registry 行。

## Stop Boundary

- 不进入 provider-specific 的 launch 上下文、home override、session 加载逻辑。
- 不修改 `ccb-providers` 中的真实后端实现。
