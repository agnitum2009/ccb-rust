# PRD: py2rust memory provider/real-context parity

## Problem
`ccb-memory` 的 parity 矩阵中 `memory` 仍标记为 `partial`。剩余两个 Python 参考测试未覆盖：

- `test_memory_transfer_providers.py`：Codex / OpenCode provider log-reader 回退提取（`extract_from_codex`、`extract_from_opencode`）。
- `test_project_memory_real_context.py`：真实 provider home-config materialization 组合（Claude / Codex / OpenCode / Gemini）生成统一的 CCB managed agent memory bundle。

## Requirements

### R1. Provider transfer parity (`test_memory_transfer_providers.py`)
1. `extract_from_codex` 在 session 文件缺失或路径无效时，回退到扫描工作目录下最近的 Codex log（`.jsonl`），并返回 conversation pairs。
2. `extract_from_opencode` 使用 OpenCode 会话状态捕获逻辑：先通过 `load_session_data` 取得 `opencode_project_id`，再扫描/捕获当前 session，最终按 `session_id` 读取对话。
3. 当 OpenCode 没有可用 session identity 时，返回 `SessionNotFoundError("No OpenCode session found")`。
4. 提取结果需经过 deduper/formatter 处理，metadata 包含实际 `session_path`。

### R2. Real-context materialization parity (`test_project_memory_real_context.py`)
1. 对 Claude / Codex / OpenCode / Gemini 四类 provider，分别 materialize 对应的运行时 memory bundle/provider-state。
2. 生成的 bundle 必须包含：
   - `# CCB Managed Agent Memory` 标题。
   - 项目级 `.ccb/ccb_memory.md` 内容（`SHARED-MEMORY-SENTINEL`）。
   - 统一的 `## CCB Runtime Coordination Rules` 块（仅出现一次）。
   - `provider: <provider>` 标记。
   - 该 agent 的 private memory（`.ccb/agents/<agent>/memory.md`）。
3. Source-home provider 原生记忆（`~/.claude/CLAUDE.md`、`~/AGENTS.md`）需被继承，但其中的 CCB 安装/roles 标记块 (`<!-- CCB_CONFIG_START -->...<!-- CCB_CONFIG_END -->` 等) 必须被过滤掉。
4. 项目根下的 provider 原生文件（`CLAUDE.md`、`AGENTS.md`、`GEMINI.md`）**不应**出现在对应 provider 的 managed memory 中（避免重复/冲突）。
5. OpenCode 的 `opencode.json` 配置需将生成的运行时 memory 路径加入 `instructions` 列表，并返回 `OPENCODE_CONFIG` env。
6. Gemini 通过 `ccb_memory::materialize_runtime_memory_bundle` 生成 bundle。

## Out of Scope
- 真正的 provider CLI log 解析（Codex CLI JSONL 格式、OpenCode 真实存储结构）。本任务仅要求与 Python 参考测试行为一致的 parity 实现/测试，允许使用 fixture/fake reader 或简化扫描。
- 修改 daemon socket / mailbox / rolepack 契约。

## Acceptance Criteria
- `cargo test -p ccb-memory -- --test-threads=1` 全部通过。
- `cargo fmt --all -- --check` 通过。
- `cargo clippy -p ccb-memory -p ccb-storage --tests` 无新增警告。
- `plans/rust-python-test-parity-matrix.md` 更新。
- 任务归档并提交。
