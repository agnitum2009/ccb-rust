# 核心类型与存储迁移（py2rust-core）

## 1. 目标

完成 CCB 核心共享层从 Python 到 Rust 的迁移收尾工作。这些 crate 已经有大量前期实现，本任务的目标是：

1. 审计现有 Rust 实现与 Python 参考实现之间的缺口；
2. 补齐影响 P0/P1 运行时的缺失功能、类型、路径布局或分类规则；
3. 更新 `plans/rust-python-test-parity-matrix.md` 中对应集群的状态；
4. 使核心 crate 达到生产就绪状态，成为上层 crate（daemon、cli、mailbox 等）的可靠基础。

## 2. 范围

### 2.1 在范围内

- `crates/ccb-runtime-env/`：环境变量解析、控制平面环境过滤、user session 环境筛选。
- `crates/ccb-types/`：共享类型与轻量 helper，主要是对 `ccb-runtime-env` 的 re-export 完整性检查。
- `crates/ccb-storage/`：存储抽象、JSON/JSONL store、路径布局、文本 artifact、锁、project identity re-export 验证。
- `crates/ccb-storage-classification/`：文件/目录分类规则、provider home 分类、runtime skills 分类。
- `crates/ccb-ui-text/`：国际化消息表、语言检测、`t()` 翻译函数，与 `lib/ui_text/i18n.py` 保持键级一致。

### 2.2 不在范围内

- `ccb-project::identity` 的具体实现修改（属于 `py2rust-project` 任务），但本任务需要验证 `ccb-storage` 对它的 re-export 是否稳定。
- 上层 daemon、cli、mailbox、terminal 的逻辑补全（分别属于后续 child tasks）。
- 新功能开发；只补齐现有行为等价所需的最小改动。

## 3. 验收标准

1. `cargo test -p ccb-runtime-env -p ccb-types -p ccb-storage -p ccb-storage-classification -p ccb-ui-text -- --test-threads=1` 全部通过。
2. `cargo fmt --check` 和 `cargo clippy --workspace` 通过（允许的 lint 除外）。
3. `plans/rust-python-test-parity-matrix.md` 中 `types_i18n`、`storage_paths` 等集群标记为 `complete` 或有明确 deferred 说明。
4. `ccb-ui-text` 中的消息键集合与 `lib/ui_text/i18n.py` 中的 `MESSAGES["en"]` 键集合一致（允许 Rust 侧多出的键，但不允许缺失被其他 crate 引用的键）。
5. `ccb-storage` 的路径布局与 Python `storage.paths.PathLayout` 在 P0/P1 路径上一致。
6. 所有核心 crate 的公共 API 有 Rust doc comments，且未使用的 `pub` 项被清理或标记为 `#[doc(hidden)]`。

## 4. 约束

- 不能破坏 `ccb-daemon`、`ccb-cli`、`ccb-mailbox` 等上层 crate 的现有调用。
- 不能改变存储路径布局，除非同时提供向后兼容的迁移逻辑。
- 不能改变控制平面协议中的环境键集合。

## 5. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| 某些 Python 功能已隐含在其他 crate 中，审计时遗漏 | 中 | 对照 parity matrix 和 Python 测试集群逐项检查 |
| 补齐功能时意外改变上层 crate 依赖的 API | 中 | 每次改动后运行 workspace 测试；必要时添加 deprecation re-export |
| 消息键不一致导致 UI 回退到 key | 低 | 写脚本对比 Python/Rust 消息键集合 |
