# Daemon 控制平面迁移设计

## 1. 现状

- `ccb-daemon` 已实现 18202 行 Rust 代码，与 Python `lib/ccbd/`（19496 行）规模接近。
- 主要子系统：
  - socket server/client、RPC 模型
  - handlers（start、stop、submit、queue、inbox、reload、project 等）
  - service graph、project namespace、health monitor、supervision
  - start/stop flow、reload apply、runtime binding

## 2. 修复项

### 2.1 `services/health.rs` 测试环境敏感性

`test_assess_provider_panes_owned_by_namespace_are_healthy` 在 tmux 环境中失败：测试期望 pane 缺失，但实际在 tmux 中 pane %1 可能被解析为存在。

修复：在测试内清空 `PATH`，使 `tmux` 命令不可见，从而 `assess_tmux_pane_state` 返回 `Missing`。

## 3. 模块映射

| Python 参考 | Rust Crate | 入口 | 状态 |
|-------------|-----------|------|------|
| `ccbd.app` / `ccbd.main` | `ccb-daemon` | `src/main.rs`, `src/app.rs` | ✅ 已覆盖 |
| `ccbd.handlers.*` | `ccb-daemon::handlers` | `src/handlers/*.rs` | ✅ 已覆盖 |
| `ccbd.services.*` | `ccb-daemon::services` | `src/services/*.rs` | ✅ 已覆盖 |
| `ccbd.start_flow` / `stop_flow` | `ccb-daemon::start_flow`, `stop_flow` | `src/start_flow.rs`, `src/stop_flow.rs` | ✅ 已覆盖 |
| `ccbd.supervision` | `ccb-daemon::supervision` | `src/supervision.rs` | ✅ 已覆盖 |
| `ccbd.reload_*` | `ccb-daemon::reload*` | `src/reload*.rs` | ✅ 已覆盖 |

## 4. 兼容性

- socket 协议保持 JSON-RPC-like，字段扩展需向后兼容。
- tmux namespace 与 pane identity 逻辑与 Python 一致。

## 5. 测试策略

- 运行 `ccb-daemon` 全部单元/集成测试。
- 全 workspace 测试确认无新增回归。
