# 核心类型与存储迁移设计

## 1. 现状

- `ccb-runtime-env`、`ccb-types`、`ccb-storage`、`ccb-storage-classification`、`ccb-ui-text` 均已存在骨架实现和测试。
- 测试当前全部通过，说明基础功能已可用。
- 主要不确定性在于：这些 crate 是否完整覆盖了 Python 参考实现中对应模块的所有 P0/P1 行为。

## 2. 模块映射

| Python 参考 | Rust Crate | Rust 入口 | 状态 |
|-------------|-----------|-----------|------|
| `lib/runtime_env/`（环境过滤） | `crates/ccb-runtime-env/` | `src/env.rs`, `src/control_plane.rs`, `src/user_session.rs` | 已实现，需审计 |
| `lib/types/` 相关 + re-export | `crates/ccb-types/` | `src/lib.rs` | 主要是 re-export，需确认完整性 |
| `lib/storage/`（JSONL store、路径） | `crates/ccb-storage/` | `src/jsonl_store.rs`, `src/paths*.rs`, `src/text_artifacts.rs` | 已实现，需审计 |
| 存储分类逻辑 | `crates/ccb-storage-classification/` | `src/classification.rs`, `src/provider_home.rs`, `src/service.rs` | 已实现，需审计 |
| `lib/ui_text/i18n.py` | `crates/ccb-ui-text/` | `src/i18n.rs` | 已实现，需审计键一致性 |

## 3. 审计策略

### 3.1 自上而下：从 parity matrix 出发

1. 读取 `plans/rust-python-test-parity-matrix.md`。
2. 确认 `types_i18n`、`storage_paths` 集群对应的 Python 测试列表。
3. 对每个 Python 测试，判断其覆盖的行为是否已有 Rust 测试或实现覆盖。
4. 缺失的覆盖项转化为本任务的补全项。

### 3.2 自下而上：从 Rust crate 出发

1. 对每个 crate，列出所有 `pub` API。
2. 对照 Python 对应模块的 `__all__` 或主要函数/类。
3. 标记 Rust 中缺失的 API 或语义差异。

### 3.3 消息键一致性检查

写一个一次性脚本或测试：

```bash
# 提取 Python 消息键
python3 - <<'PY'
import ast, json
with open('lib/ui_text/i18n.py') as f:
    tree = ast.parse(f.read())
for node in ast.walk(tree):
    if isinstance(node, ast.Assign) and any(t.id == 'MESSAGES' for t in node.targets):
        msgs = ast.literal_eval(node.value)
        print(json.dumps(sorted(msgs['en'].keys())))
        break
PY

# 提取 Rust 消息键（通过 grep/regex 解析 i18n.rs 中的 HashMap）
grep -E '^\s*\("[a-z_]+"' rust/crates/ccb-ui-text/src/i18n.rs | sed -E 's/.*\("([a-z_]+)".*/\1/' | sort -u
```

如果 Python 键集合不是 Rust 键集合的子集，则缺失的键需要补充。

## 4. 补全策略

- **小缺口**：直接在对应 crate 中补充函数/测试。
- **中缺口**：如果涉及多个 crate，先在 design.md 中记录边界，再分文件修改。
- **大缺口**：如果某 Python 模块整体未迁移，则在本任务的 implement.md 中记录，并考虑是否拆出新的 child task。

## 5. 兼容性

- 所有新增公共 API 使用 `#[non_exhaustive]` 或谨慎添加，避免未来改动破坏上层。
- 不删除已有 `pub` API；若确实冗余，先标记 `#[deprecated]`。
- 路径布局变更必须伴随 workspace 范围测试验证。

## 6. 测试策略

- 单元测试：每个新增/修改函数必须有单元测试。
- 集成测试：涉及存储路径、分类、环境解析的，使用临时目录做集成测试。
- 一致性测试：`ccb-ui-text` 增加消息键一致性测试（可选在 CI 中运行）。
- 回归测试：每次提交前运行目标 crate 和全 workspace 测试。
