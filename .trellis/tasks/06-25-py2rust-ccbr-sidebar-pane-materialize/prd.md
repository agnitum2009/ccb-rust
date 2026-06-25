# ccbr sidebar pane 物化缺失（topology 不建左侧 sidebar pane）

## Goal

ccbr parses [ui.sidebar] but topology/materialize never creates the left sidebar pane (only agent panes). Port Python sidebar pane materialization: each window gets a left 15% pane launching ccbr-agent-sidebar. Mouse on + ccbr-agent-sidebar symlink already in place.

## Requirements

- TBD

## Acceptance Criteria

- [ ] TBD

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.

## 精确根因（glm5.2 实查，2026-06-25）
- `[ui.sidebar]` 配置 **加载正确**（`ccbr config validate` = ok；`config.rs:498` 读 `[ui.sidebar]`→`ProjectConfig.sidebar`）。
- `build_namespace_topology_plan` + `window_plan` **正确把 sidebar plan 挂到每个 window**（`topology_plan.rs:148`）。
- `_materialize_sidebar`（`materialize_topology.rs:386`）逻辑**正确**（split root_pane 向右 → 左侧成 sidebar + `_respawn_sidebar` 启 `ccbr-agent-sidebar`）。
- **真根因**：`ccbr start` 走 `start_flow/service.rs`，它**自定义创建 agent pane，不调 `materialize_topology`**。sidebar 物化只在 `materialize_topology` 里，仅被 `ensure.rs:79`（namespace ensure）+ `reload_apply_namespace.rs:59`（reload）调用，**没接入初始 start 流程**。Wave 4 Layer 2 的 live e2e 重写 start_flow（remain-on-exit/stale-pane）时 sidestep 了 namespace materialize。

## 修复点（任选其一，推荐第 1）
1. **start_flow/service.rs 建 agent pane 后，按 `[ui.sidebar] every_window` 为每个 window 调 sidebar 物化**（split 左 pane + 启 `ccbr-agent-sidebar`）—— 复用 `materialize_topology.rs` 的 `_materialize_sidebar` 逻辑（提取为 pub helper 或在 start_flow 内镜像）。
2. 或让 `ccbr start` 在 start_flow 后调一次 `materialize_topology`（namespace ensure 路径），使其补建 sidebar pane（注意不与 start_flow 已建 pane 重复/冲突）。

## 已就位的前置（不用再做）
- `ccbr-agent-sidebar` 软链 → Python `ccb-agent-sidebar`（`/usr/local/bin/ccbr-agent-sidebar`，PATH 可见）。
- `run-ccbr.sh up` 已设 `tmux set -g mouse on`（鼠标切换已生效）。
- dapro-ass `.ccbr/ccbr.config` 已有 `[ui.sidebar]` + `[ui.sidebar.view]`。

## 验收
- `ccbr start` 后每个 window 最左出现 ~15% 宽 sidebar pane，渲染 agent 列表（Python `ccb-agent-sidebar` 经软链画）。
- 不破坏 Wave 4 Layer 2 的 start_flow（remain-on-exit/stale-pane/respawn）。
- `cargo test -p ccbr-daemon` 全绿。
