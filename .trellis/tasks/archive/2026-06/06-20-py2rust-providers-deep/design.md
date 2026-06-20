# Provider Registry 静态映射设计

## 变更点

1. `rust/crates/ccb-provider-core/src/registry.rs`
   - 新增私有辅助函数 `session_binding_for_provider(provider)`：
     - 返回该 provider 默认的 `ProviderSessionBinding`。
     - `opencode` 使用 `session_path_attr = "session_file"`。
     - 其他已知 provider 使用 `<provider>_session_id` / `<provider>_session_path`。
     - 未知 provider 返回 `None`。
   - 新增私有辅助函数 `runtime_launcher_for_provider(provider)`：
     - `codex` 返回 `LaunchMode::CodexTmux`。
     - 其他已知 provider 返回 `LaunchMode::SimpleTmux`。
     - 未知 provider 返回 `None`。
   - 修改 `build_default_session_binding_map(include_optional)`：
     - 遍历 `CORE_PROVIDER_NAMES` 与（当 `include_optional` 为真时）`OPTIONAL_PROVIDER_NAMES`。
     - 使用辅助函数生成绑定并插入 `HashMap`。
   - 修改 `build_default_runtime_launcher_map(include_optional)`：
     - 同样的遍历逻辑，生成 launcher 并插入 `HashMap`。

2. `rust/crates/ccb-provider-core/tests/registry_tests.rs`
   - 测试 `build_default_session_binding_map(true)` 的 key set 与 attr 值。
   - 测试 `build_default_runtime_launcher_map(true)` 的 key set 与 mode 值。
   - 测试 `include_optional=false` 时仅含 core provider。
   - 补充 `session_filename_for_agent` 对 codex/agy/kimi/deepseek 的命名断言。

## 兼容性

- `build_default_backend_registry` 与 `ProviderBackendRegistry` 内部仍保持 manifest-only；不影响 `ccb-providers` 的完整后端注册。
- 未知 provider 在辅助函数中返回 `None`，因此 `EXTRA_PROVIDER_NAMES` 不会进入这两个静态 map。
