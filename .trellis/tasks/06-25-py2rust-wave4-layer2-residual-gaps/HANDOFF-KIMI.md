# Handoff — Wave 4 Layer 2 残留 gap 闭环 → kimi2.7

> Prepared for: **kimi2.7** | From: glm5.2（审核方） | Branch: `python-rust/rolepacks-versioning-translation`
> 上游：归档任务 s3-multi-recovery / s4-edge（commit `0daaf4da`）。本任务闭环其残留 gap。

## glm5.2 审核结论（已核验实物，非轻信报告）
- ✅ `0daaf4da` 属实：`app.rs`(+56) `respawn_dead_agents` + `project_restart.rs`(+47) 非.stub，代码扎实。
- ✅ 产品仓 `bdcc5ad` ff-push（local==origin/master）；ccb-legacy `d67cf250` 2 文件反向重命名、独立血系。
- ⚠️ **S4 零新代码**（仅 research 验证）；**S3 新代码无单测**（244/164 passed 是既有套件）。
- ⚠️ ccb-legacy 7 个历史文档含 "ccbr" → 经抽样确认是**描述重命名的规划文档**，**保留不改动**（见 prd Notes）。

## 5 处 gap（逐项闭环，证据入 research/）

### S3.3 daemon 重启 job 连续性
- 现状：daemon 重启不重注册 running jobs，trace 空（reply 事件已持久化，无孤儿）。
- 方向：startup 路径加载持久化 job/mailbox 状态 → 重注册 Running jobs 到 execution/heartbeat。
- 关键代码：`ccbr-daemon/src/app.rs`（startup/heartbeat）、`ccbr-storage`（持久化）。

### S3 新代码单测
- 复用 ccbr-daemon 既有 tmux 测试 harness（见现 244 个测试的 tests/ 目录模式）。
- `respawn_dead_agents`：构造死 pane → 验证触发 run_start_flow；活 pane → no-op。
- `handle_project_restart_agent`：agent 未配置 → failed；正常 → 调 run_start_flow(restore=false)。

### S4.4 真正 mid-run cancel
- 用慢 provider（如 opencode 或加 system-prompt 延迟）/ 注入 sleep，使 job 在 cancel 时仍在运行，验证 `ccbr cancel <job-id>` 真正截停 + agent idle。

### S4.5 timeout live
- 人为阻塞 provider（断网/stall），验证 timeout 优雅触发。

### S4.6 auth 失败 CLI 错误
- 临时移除 `$PROJECT/.ccbr/runtime/<agent>/home/auth.json` → restart → CLI 应收到**明确 auth 错误**（非静默无 hang）。
- 关键代码：`ccbr-provider-profiles/src/codex_home_config.rs:materialize_auth_file()` + provider 启动错误传播到 `ccbr-cli`。

## 运行方式
同 S3/S4 HANDOFF（`export CCBR_SOURCE_RUNTIME_OK=1`，干净状态清理 + daemon/start，socket/pane 动态取）。测试环境 `/mnt/d/dapro-ass`。

## 完成判定
5 项全部闭环 + 证据；新代码同步 ccb-legacy（反向重命名 ccbr→ccb，方法见 ccb-legacy-nonrust-sync 归档 HANDOFF）+ 产品仓 ff-push（`/home/agnitum/ccb-rust-prod-workflow.md`）。
