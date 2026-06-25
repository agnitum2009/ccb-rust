# ccb-legacy 非 rust/ 反向重命名同步（docs/scripts）

## Goal
把 ccbr HEAD 的非 rust/ 部分（docs/scripts/plans/根配置）以反向重命名 ccbr→ccb 同步到 ccb-legacy，补齐"仅命名不同"对齐（rust/ 已由 commit 547e91e5 同步）。ccb-legacy 为独立双生血系，永不与 ccbr 合并。

## Requirements
- 比对 ccbr HEAD 与 ccb-legacy 在 docs/、scripts/、plans/、根级配置的非 rust/ 差异
- 反向重命名（ccbr→ccb、CCBR→CCB，内容+路径）同步到 ccb-legacy
- 保留 ccb-legacy 自有 `.trellis/`（任务管理不覆盖）
- ccbr 产品仓专属文件（`ccb-rust-prod-workflow.md`、产品仓 README/引用）按需保留/适配，不盲目同步
- 同步后非 rust/ 部分：零 ccbr 泄漏、内容与 ccbr 一致（除命名）

## Acceptance Criteria
- [ ] docs/scripts/plans（适用部分）同步到 ccb-legacy（反向重命名），零 ccbr 泄漏
- [ ] ccb-legacy 自有 `.trellis/` 完整保留
- [ ] ccb-legacy 仍为独立血系（不与 ccbr 互为祖先）
- [ ] 提交到 ccb-legacy 分支

## Notes
- rust/ 已同步（547e91e5），本任务只处理非 rust/。PRD-only 可启动。方法见 HANDOFF-KIMI.md。
