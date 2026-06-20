# py2rust-core 审计报告

审计时间：2026-06-20
审计范围：`ccb-runtime-env`、`ccb-types`、`ccb-storage`、`ccb-storage-classification`、`ccb-ui-text`

## 1. 总体结论

核心共享层 Rust 实现已经高度完整，与 Python 参考实现行为基本一致。本任务以**验证、文档更新和少量收尾**为主，不需要大规模重写。

## 2. 逐项审计

### 2.1 `ccb-runtime-env`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| `runtime_env/__init__.py`: `env_bool` | `src/env.rs`: `env_bool` | ✅ 一致 | 真/假值集合相同 |
| `runtime_env/__init__.py`: `env_int` | `src/env.rs`: `env_int` | ✅ 一致 | |
| `runtime_env/__init__.py`: `env_float` | `src/env.rs`: `env_float` | ✅ 一致 | |
| `runtime_env/control_plane.py`: `control_plane_env` | `src/control_plane.rs`: `control_plane_env` | ✅ 一致 | Rust 额外转发 `PROVIDER_START_ENV_VARS`，属于增强而非缺失 |
| `runtime_env/user_session.py`: 常量集 | `src/user_session.rs`: 常量集 | ✅ 一致 | 网络代理、信任库、桌面会话、WSL 键完全一致 |
| `runtime_env/user_session.py`: `user_session_transport_env` | `src/user_session.rs`: `user_session_transport_env` | ✅ 一致 | |

### 2.2 `ccb-types`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| 共享类型与 re-export | `src/lib.rs` re-export `ccb_runtime_env` | ✅ 一致 | 当前作为轻量兼容层存在 |
| `src/error.rs`, `src/ui.rs` | 本地定义 | ✅ 存在 | 需确保上层调用稳定 |

### 2.3 `ccb-storage`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| `storage.paths.PathLayout` | `src/paths.rs`: `PathLayout` | ✅ 已实现 | 内存、共享缓存、运行时状态等路径完整 |
| JSON/JSONL store | `src/json_store.rs`, `src/jsonl_store.rs` | ✅ 已实现 | append/read_all/read_tail/find_last |
| 文本 artifacts | `src/text_artifacts.rs` | ✅ 已实现 | spill/sweep/validate |
| 锁 | `src/locks.rs` | ✅ 已实现 | |
| project identity | `src/project_identity.rs` re-export `ccb_project::identity` | ✅ 已委托 | 实际实现在 `ccb-project` |

### 2.4 `ccb-storage-classification`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| `StorageClass` 枚举 | `src/classification.rs` | ✅ 已实现 | 11 个分类与 Python 一致 |
| Provider home 分类 | `src/provider_home.rs` | ✅ 已实现 | |
| 分类服务 | `src/service.rs` | ✅ 已实现 | |

### 2.5 `ccb-ui-text`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| 消息键集合 | `src/i18n.rs` | ✅ 一致 | Python 53 个键，Rust 53 个键，无缺失 |
| 语言检测 | `detect_language` | ✅ 一致 | 优先级相同 |
| `t()` 翻译 | `t()` | ✅ 一致 | 占位符替换行为相同 |

## 3. 发现的问题

### 3.1 当前 workspace baseline 失败

```
crates/ccb-daemon/src/services/health.rs:361
services::health::tests::test_assess_provider_panes_owned_by_namespace_are_healthy
expected: Alive, got: Missing
```

- **影响**：不影响 py2rust-core 目标 crate 的单独测试，但导致 `cargo test --workspace` 失败。
- **归属**：属于 `py2rust-daemon` / `py2rust-terminal` 范围。
- **处理**：记录为跨 task baseline 问题，不在本 task 修复。

### 3.2 `control_plane_env` 的 `PROVIDER_START_ENV_VARS`

Rust 额外转发 provider start override 环境变量，Python allowlist 中没有显式包含。这是 Rust 实现比 Python 更完整的表现，不影响兼容性。

## 4. 建议的收尾工作

1. 更新 `plans/rust-python-test-parity-matrix.md`：
   - 在 `types_i18n` 和 `storage_paths` 的 Notes 中说明核心层已完成，provider 协议/路径部分由后续 child tasks 覆盖。
2. 为核心 crate 增加消息键一致性测试（可选，已手动验证无缺失）。
3. 确认 `ccb-types` 的 re-export 在未来上层 crate 需要新增类型时有扩展空间。

## 5. 验证结果

```bash
cargo test -p ccb-runtime-env -p ccb-types -p ccb-storage -p ccb-storage-classification -p ccb-ui-text -- --test-threads=1
# 结果：全部通过

cargo test --workspace -- --test-threads=1
# 结果：1 个 baseline 失败（ccb-daemon health test），与核心层无关
```
