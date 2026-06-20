# Terminal 与 Pane Registry 迁移设计

## 1. 现状

- `ccb-terminal` 已建立大量模块，覆盖：
  - tmux backend 控制、panes、layouts、identity、detect、theme、logs、respawn。
  - 但部分模块仍是 stub（如 `pane_logs_runtime/trim.rs`、`api_selection.rs`），导致大量 unused/dead-code lint。
- `ccb-pane-registry` 已有完整实现和测试。

## 2. 修复项

### 2.1 `detect.rs` 测试环境敏感性

`test_client_tty_matches_false_without_tmux` 在 tmux 环境中失败，因为 `client_tty_matches` 直接调用 `tmux display-message`。修复：在测试内将 `PATH` 置空，使 `tmux` 命令找不到。

### 2.2 `ptr_arg` lint

`pane_logs_runtime/trim.rs` 和 `pane_logs_runtime/paths.rs` 使用 `&PathBuf` 参数，改为 `&Path`。

### 2.3 stub 模块 lint

`api_selection.rs`、`backend_selection.rs` 等 stub 模块存在 unused variables、dead_code、new_without_default、too_many_arguments。在 `lib.rs` 顶部添加临时 `#![allow(...)]` 抑制这些 lint，待模块实现后移除。

## 3. 模块映射

| Python 参考 | Rust Crate | 入口 | 状态 |
|-------------|-----------|------|------|
| `terminal_runtime.detect` | `ccb-terminal::detect` | `src/detect.rs` | ✅ 修复测试后通过 |
| `terminal_runtime.tmux_*` | `ccb-terminal::tmux*` | `src/tmux*.rs` | ✅ 已覆盖 |
| `terminal_runtime.pane_logs_runtime` | `ccb-terminal::pane_logs_runtime` | `src/pane_logs_runtime/` | ⚠️ stub，lint 已抑制 |
| `pane_registry_runtime` | `ccb-pane-registry` | `src/*.rs` | ✅ 已覆盖 |

## 4. 兼容性

- tmux 命名空间、pane identity、session name 等核心约定保持与 Python 一致。

## 5. 测试策略

- 运行目标 crate 测试。
- 特别关注在 tmux 环境中运行的 detect 测试。
