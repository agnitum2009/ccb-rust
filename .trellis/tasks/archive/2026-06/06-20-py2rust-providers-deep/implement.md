# Provider Registry 静态映射执行计划

## 实施步骤

1. **修改 `rust/crates/ccb-provider-core/src/registry.rs`**
   - 在 `build_default_session_binding_map` 与 `build_default_runtime_launcher_map` 上方添加辅助函数 `session_binding_for_provider` 和 `runtime_launcher_for_provider`。
   - 使用 `CORE_PROVIDER_NAMES` / `OPTIONAL_PROVIDER_NAMES` 常量生成 map。
   - 移除原有的空 map 注释与实现。

2. **新建 `rust/crates/ccb-provider-core/tests/registry_tests.rs`**
   - 引用 `ccb_provider_core::registry` 与 `ccb_provider_core::pathing`。
   - 编写 `test_default_session_binding_map`：
     - assert key set 等于 8 个 provider。
     - assert codex session_id_attr / opencode session_path_attr / agy/kimi/deepseek session_path_attr。
   - 编写 `test_default_runtime_launcher_map`：
     - assert key set 等于 8 个 provider。
     - assert codex launch_mode / gemini/agy/kimi/deepseek launch_mode。
   - 编写 `test_session_binding_map_core_only`：
     - assert `include_optional=false` 时只有 codex/claude/gemini。
   - 编写 `test_session_filename_for_agent_provider_variants`：
     - assert codex/writer、agy/antigravity、kimi/moon、deepseek/coder 的命名。

3. **验证**
   - `cargo test -p ccb-provider-core -- --test-threads=1`
   - `cargo clippy -p ccb-provider-core -- -D warnings`
   - `cargo fmt -p ccb-provider-core`

4. **更新 parity matrix**
   - 在 `docs/gap-reports/` 或 `.trellis/tasks/...` 的 parity 记录中标记 `test_v2_provider_core_registry.py` 已对齐。

## 停止边界

- 不修改 `ccb-providers` 的真实后端、launcher 上下文、session 加载逻辑。
- 不改动 mailbox、daemon、terminal 等其他 crate。
