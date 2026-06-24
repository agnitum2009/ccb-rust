# Design: py2rust memory provider/real-context parity

## Boundaries
- 修改范围：`ccb-memory`（transfer / transfer_runtime / 测试）、`ccb-provider-profiles`（`codex_home_config.rs`）、`ccb-providers`（`claude/launcher_runtime/home.rs`、`opencode/launcher.rs`）。
- 不改动 `ccb-daemon`、`ccb-mailbox`、`ccb-heartbeat` 等 socket/控制平面。

## Data Flow

### Provider transfer extraction
```
extract_from_codex(work_dir, ...)
  ├─ load_session_data(work_dir, "codex") -> (session_file, data)
  ├─ if explicit session_path invalid -> fallback scan latest .jsonl log in work_dir
  └─ read conversation pairs -> dedupe -> format -> TransferContext

extract_from_opencode(work_dir, source_session_files, ...)
  ├─ load_session_data(work_dir, "opencode") -> (session_file, data)
  ├─ data["opencode_project_id"] -> project_id
  ├─ scan work_dir/.opencode/sessions or similar for latest session
  ├─ if no session identity -> SessionNotFoundError
  └─ read conversation pairs -> dedupe -> format -> TransferContext
```

### Managed provider memory bundle
统一由 `ccb-memory` 提供 `render_managed_agent_memory(project_root, agent_name, provider, source_memory_path, workspace_path)`：
1. 读取 `.ccb/ccb_memory.md`。
2. 读取 agent private memory `.ccb/agents/<agent>/memory.md`。
3. 读取 source-home provider 原生 memory（如 `~/.claude/CLAUDE.md`、`~/AGENTS.md`）。
4. 使用 `ccb-memory::filter_memory_source` 过滤 source memory 中的 CCB 安装/roles 标记块。
5. 按固定模板拼接：
   - `# CCB Managed Agent Memory`
   - `provider: <provider>`
   - `## Project Memory`
   - `## Agent Private Memory`
   - `## Provider User Memory`
   - `## CCB Runtime Coordination Rules`
6. 返回渲染文本和内存投影事件结果。

Claude / Codex / Gemini / OpenCode materializer 调用上述渲染函数写入目标路径；OpenCode 额外将 `.ccb/runtime/memory/<agent>.md` 加入 `opencode.json` instructions。

## Compatibility
- 现有 `ccb-memory` integration tests 已通过；新增行为不应破坏已有 filter/ensure/materialize 测试。
- `materialize_provider_memory_file` 当前输出 `# Provider User Memory` 标题；本设计将其替换为统一 managed header，属于行为变更，但 parity 目标要求与 Python 一致。

## Rollback
- 若 provider materialization 的标题变更影响其他 crate 测试，可在 `render_managed_agent_memory` 中通过 feature flag / provider 参数保留旧标题，但优先统一。
