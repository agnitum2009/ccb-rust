# Wave 3 Python↔Rust 一致性收尾 — 执行进度（细粒度）

> 任务：`.trellis/tasks/06-24-py2rust-consistency-closure`
> 分支：`python-rust/rolepacks-versioning-translation`
> 最后更新：2026-06-20

---

## 1. callbacks 子系统（daemon_lifecycle）

| Python 测试/行为 | Rust 实现位置 | 测试位置 | 状态 |
|---|---|---|---|
| callback-edge 注册 (`register_callback_edge`) | `ccb-daemon/src/services/dispatcher.rs` | `ccb-daemon/tests/callbacks_tests.rs` | ✅ |
| continuation-job 提交 (`submit_callback_continuation`) | 同上 | 同上 | ✅ |
| callback-chain 深度/环校验 (`_validate_callback_chain`) | 同上 | 同上 | ✅ |
| nested-ask 路由/校验 | 同上 | 同上 | ✅ |
| timeout sweep / repair / fail / mark done | 同上 | 同上 | ✅ |
| delegated-terminal 持久化 | 同上 | 同上 | ✅ |
| 12 个核心 scenario + 3 个 edge | 同上 | 同上 | ✅ 15 tests pass |

**提交**：`de1602a9`

---

## 2. terminal_runtime 集群

| Python 测试/行为 | Rust 实现位置 | 测试位置 | 状态 |
|---|---|---|---|
| `should_attach_selected_pane` 修复 | `ccb-terminal/src/tmux_attach.rs` | `ccb-terminal/tests/tmux_attach_tests.rs` | ✅ |
| `normalize_user_option` / pane alive helpers | 同上 | 同上 | ✅ |
| `CCB_TMUX_CONFIG` / `-S` socket 覆盖 | `ccb-terminal/src/tmux_attach.rs` | `ccb-terminal/tests/test_tmux_helpers.rs` | ✅ |
| tmux 三类 transient failure retry | `ccb-terminal/src/respawn.rs` | `ccb-terminal/tests/test_respawn.rs` | ✅ |
| shared-ready-budget respawn | 同上 | 同上 | ✅ |
| stderr-redirection parent-dir | 同上 | 同上 | ✅ |
| log-refresh dead/pipe | `ccb-terminal/src/logs.rs` | `ccb-terminal/tests/test_respawn.rs` | ✅ |
| paste-buffer cleanup-on-failure | `ccb-terminal/src/input.rs` | `ccb-terminal/tests/test_tmux_helpers.rs` | ✅ |

**提交**：`37aa0b68`

---

## 3. providers 次要子特性

### 3.1 active_runtime（resume/start）

| Python 测试/行为 | Rust 实现位置 | 测试位置 | 状态 |
|---|---|---|---|
| `resume_active_submission_requires_active_workspace` | `ccb-providers/src/active_runtime/resume.rs` | inline unit tests | ✅ |
| `resume_active_submission_skips_passive_runtime_state` | 同上 | 同上 | ✅ |
| `resume_active_submission_restores_reader_backend_and_completion_dir` | 同上 | 同上 | ✅ |
| `_session_selector_name`（instance/agent/provider fallback） | `ccb-providers/src/active_runtime/start.rs` | 同上 | ✅ |
| `prepare_active_start` 错误/成功路径 | 同上 | 同上 | ✅ |
| `PreparedActiveStart` / `PreparedActivePoll` 模型 | `ccb-providers/src/active_runtime/models.rs` | 同上 | ✅ |

**新增测试数**：10
**提交**：`a88ccd24`

### 3.2 opencode

| Python 测试/行为 | Rust 实现位置 | 测试位置 | 状态 |
|---|---|---|---|
| SQLite `_read_messages` / `_read_parts` | `ccb-providers/src/opencode/reader.rs` | inline unit tests | ✅ |
| SQLite → JSON fallback | 同上 | 同上 | ✅ |
| `_get_latest_session_from_db` 按 session filter 固定 | 同上 | 同上 | ✅ |
| `OpenCodeStorageAccessor::fetch_opencode_db_rows` | `ccb-providers/src/opencode/storage.rs` | 同上 | ✅ |
| `ensure_pane` respawn / alive / no backend / dead no marker / missing target | `ccb-providers/src/opencode/session.rs` | 同上 | ✅ |
| `initialize_state` 填充 runtime 字段 | `ccb-providers/src/opencode/runtime/communicator.rs` | 同上 | ✅ |
| `initialize_state` session missing 抛异常 | 同上 | 同上 | ✅ |
| `_load_session_info` backfill + `_find_session_file` CCB_SESSION_FILE | `ccb-providers/src/opencode/comm.rs` | 同上 | ✅ |

**新增测试数**：11（reader 3 + session 5 + communicator 2 + comm 2）
**新增依赖**：`rusqlite = { version = "0.31", features = ["bundled"] }`
**未提交**：集成在该轮次最后统一提交

### 3.3 claude_registry（进行中）

| Python 测试文件 | 目标 Rust 模块 | 状态 |
|---|---|---|
| `test_claude_registry_cache.py` | `claude/registry_runtime/{state,cache_runtime/*}.rs` | 🔄 实现中 |
| `test_claude_registry_events.py` | `claude/registry_runtime/events*.rs` | 🔄 实现中 |
| `test_claude_registry_logs_binding.py` | `claude/registry_support/logs_runtime/binding.rs` | 🔄 实现中 |
| `test_claude_registry_logs_discovery.py` | `claude/registry_support/logs_runtime/discovery*.rs` | 🔄 实现中 |
| `_find_claude_session_file` / `_load_claude_session` | `claude/registry.rs` | 🔄 实现中 |

**阻塞/风险**：该子系统 Python 代码量最大（cache + events + 4 个 logs 子模块），需要逐个函数对齐；当前已梳理完测试文件，开始写实现。

---

## 4. 全局质量门

| 检查 | 当前状态 | 备注 |
|---|---|---|
| `cargo test -p ccb-providers --lib` | ✅ 248 passed | 含新增 opencode/active_runtime |
| `cargo test -p ccb-daemon --lib` | ✅ 通过 | callbacks + 原有 |
| `cargo test -p ccb-terminal --lib` | ✅ 通过 | terminal_runtime |
| `cargo clippy -p ccb-providers --all-targets` | 🔄 清理中 | 剩余若干既有 needless_borrow / manual contains |
| `cargo clippy --workspace --all-targets` | 🔄 清理中 | 剩余 6 个错误（ccb-terminal 4 个 + ccb-providers 2 个） |
| `cargo fmt --check` | ✅ 已 fmt | 持续保持 |
| `plans/rust-python-test-parity-matrix.md` | ⏳ 待更新 | claude_registry 完成后统一更新 |

---

## 5. 下一步动作

1. 实现 `claude/registry_runtime/state.rs`：`SessionEntry`、`WatcherEntry`、`RegistryRuntimeState`。
2. 实现 `cache_runtime/loading.rs`、`lookup.rs`、`mutation.rs`，覆盖 cache 5 个测试。
3. 实现 `events_runtime/{common,global_logs,project_logs,sessions_index}.rs`，覆盖 events 3 个测试。
4. 新建 `registry_support/logs_runtime/`，实现 binding / discovery / extract / lookup / scan / indexing / meta，覆盖 logs 7 个测试。
5. 扩展 `claude/registry.rs` 的 `_find_claude_session_file` / `_load_claude_session`。
6. 清理剩余 clippy 错误，跑 `cargo test --workspace`，更新 parity matrix，提交。
