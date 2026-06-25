# Wave 4 Layer 2 残留 gap 闭环（S3.3 / S3 测试 / S4.4 / S4.5 / S4.6）

## Goal
闭环 S3/S4 live e2e 遗留、kimi 自报 + glm5.2 审核确认的 5 处 gap，使 Wave 4 Layer 2 live 覆盖真正完整。

## Requirements（每项可独立验收）
- **S3.3 daemon 重启 job 连续性**：daemon 重启时从持久化状态重注册 Running jobs，trace 不再为空（reply 事件已持久化、无孤儿，仅 running 状态丢失）。
- **S3 新代码单测**：为 `respawn_dead_agents()`（`app.rs`）与 `handle_project_restart_agent`（`project_restart.rs`，commit `0daaf4da`）补单测，复用 ccbr-daemon 既有 tmux 测试 harness（现 244 测试）。覆盖：死 pane 检测、respawn 触发 run_start_flow、agent 未配置拒绝、restore=false。
- **S4.4 真正 mid-run cancel**：用更慢 provider 或注入延迟，演示 cancel 在 job 运行中真正截停（非响应过快没截到）。
- **S4.5 timeout live 测试**：live 验证 timeout 路径（人为阻塞/stall 注入 provider）。
- **S4.6 auth 失败 CLI 错误显式化**：auth 失败时向 CLI 显式返回 auth 错误（现状：无 hang 但无显式错误）。

## Acceptance Criteria
- [ ] S3.3 daemon 重启后 running jobs 在 trace 可见
- [ ] respawn_dead_agents + project_restart 有单测且通过
- [ ] S4.4 演示一次真正 mid-run cancel 成功
- [ ] S4.5 timeout 路径有 live 证据
- [ ] S4.6 auth 失败时 CLI 收到明确 auth 错误
- [ ] 新代码同步 ccb-legacy（反向重命名）+ 产品仓 ff-push

## Notes
- 审核特征化（glm5.2，非 gap、仅记录）：ccb-legacy 的 docs/plans 中 7 个含 "ccbr" 的文件是**描述 ccb→ccbr 重命名的历史规划文档**（如"执行入口已统一改为 ccbr"），**非代码泄漏**（ccb-legacy 代码仍 ccb-*，cargo check 干净）。**决定：保留作历史记录，不反向重命名**（反重命名会篡改历史叙事）。S4 为"验证既有行为"，未产新代码，属预期。
