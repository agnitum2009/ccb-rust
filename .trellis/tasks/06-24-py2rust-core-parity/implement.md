# Wave 2 核心 parity 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐 runtime launch 编排、completion/heartbeat/job-store 剩余 parity、以及 CLI maintenance 编排，使对应 Rust 测试能够覆盖 Python v7.5.2 的等价场景。

**Architecture：** 在现有 `ccb-daemon::start_runtime`、`ccb-completion`、`ccb-heartbeat`、`ccb-jobs`、`ccb-cli` 的已实现骨架上，注入缺失的边界行为（detached fallback、foreign binding 拒绝、maintenance tick/runner/schedule），全部通过新增单元/集成测试验证，不引入新的外部依赖。

**Tech Stack：** Rust 2021, `serde_json`, `camino`, `tempfile`（测试）, `cargo test -- --test-threads=1`。

---

## File structure

| File | Responsibility |
|------|----------------|
| `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs` | 实现 `EnsureAgentRuntimeImpl` 的 detached fallback、pane 最小尺寸、namespace limits、foreign binding 拒绝。 |
| `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs` | 调整 `compute_launch_binding_hint` 以识别 foreign socket/project。 |
| `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs` | 扩展 `RuntimeBinding` 字段（identity state、project id）。 |
| `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs` | 新增 detached/foreign/namespace/size 测试。 |
| `rust/crates/ccb-completion/tests/integration_tests.rs` | 新增 `SessionRotate` selector reset 测试。 |
| `rust/crates/ccb-heartbeat/src/classifier.rs` | 清理空 stub，re-export `maintenance.rs` 的公开函数。 |
| `rust/crates/ccb-heartbeat/tests/integration.rs` | 新增 classifier stub 清理的编译/行为检查。 |
| `rust/crates/ccb-jobs/tests/store_integration.rs` | 新增事件日志跳过非 `job_event` 记录测试。 |
| `rust/crates/ccb-cli/src/services/maintenance.rs` | 实现 `maintenance_status/tick/schedule/runner`。 |
| `rust/crates/ccb-cli/src/commands.rs` | 将 `maintenance` 命令路由到 service 层。 |
| `rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs` | 扩展 `render_maintenance` 输出字段。 |
| `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs` | 新增 CLI maintenance 集成测试。 |
| `plans/rust-python-test-parity-matrix.md` | 更新测试映射与状态。 |

---

## Task 1: Runtime launch — detached fallback + pane size + namespace limits

**Files:**
- Modify: `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs`
- Modify: `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs`
- Test: `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`

- [ ] **Step 1: 写失败测试 `test_pane_too_small_triggers_detached_fallback`**

  在 `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs` 末尾新增：

  ```rust
  #[test]
  fn test_pane_too_small_triggers_detached_fallback() {
      let tmp = tempfile::tempdir().unwrap();
      let root = tmp.path().to_string_lossy().to_string();
      let mut context = Context {
          project_id: "proj".into(),
          project_root: root.clone(),
          workspace_path: root.clone(),
      };

      let backend = Arc::new(FakeBackend::new("%99"));
      // 第一次 create_pane 返回 %1，尺寸检查失败
      backend.mark_alive("%1", true);
      let impl_ = make_ensure_impl_with_min_size(backend.clone(), 80, 24);

      let result = EnsureAgentRuntimeFn::call(
          &impl_,
          &context,
          &Command { restore: false },
          &AgentSpec { name: "agent1".into(), runtime_mode: "pane".into(), provider: "codex".into() },
          &Plan { workspace_path: root },
          None,
          None,
          0,
          Some("/tmp/tmux.sock"),
      )
      .unwrap();

      assert!(result.launched);
      assert_eq!(result.binding.unwrap().runtime_ref.as_deref(), Some("tmux:%99"));
      assert!(backend.has_call("tmux_run:kill-pane"));
      assert!(backend.has_call("tmux_run:new-session"));
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-daemon test_pane_too_small_triggers_detached_fallback -- --test-threads=1
  ```
  期望：编译失败（`make_ensure_impl_with_min_size` 不存在）。

- [ ] **Step 2: 扩展 `EnsureAgentRuntimeImpl` 支持 `allow_detached_fallback` 和最小尺寸**

  在 `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs` 中：

  1. 给 `EnsureAgentRuntimeImpl` 增加字段：
     ```rust
     pub struct EnsureAgentRuntimeImpl {
         launcher: ProviderLauncher,
         backend_factory: TmuxBackendFactory,
         allow_detached_fallback: bool,
         min_pane_width: u32,
         min_pane_height: u32,
     }
     ```
  2. 更新 `new`/`with_default_backend` 默认 `allow_detached_fallback: true, min_pane_width: 20, min_pane_height: 8`。
  3. 新增 builder 方法：
     ```rust
     pub fn with_allow_detached_fallback(mut self, allow: bool) -> Self { self.allow_detached_fallback = allow; self }
     pub fn with_min_pane_size(mut self, width: u32, height: u32) -> Self { self.min_pane_width = width; self.min_pane_height = height; self }
     ```
  4. 在 `call` 的 pane 创建分支中：
     - 先尝试 `backend.create_pane(...)` 得到 `pane_id`。
     - 调用新增 `pane_meets_minimum_size(&*backend, &pane_id, self.min_pane_width, self.min_pane_height)`。
     - 若尺寸不足，调用 `best_effort_kill_pane` 后，若 `allow_detached_fallback` 为 true 则走 `create_detached_tmux_pane`，否则返回 `Err("project namespace launch could not allocate stable tmux pane for {agent_name}".into())`。
     - 若 `create_pane` 错误信息包含 `split-window failed` 或 `no space for new pane`（大小写不敏感），同样走 detached fallback（仅当 `allow_detached_fallback=true`）。

- [ ] **Step 3: 实现 detached pane 创建和 server 准备**

  在同一文件中新增函数：

  ```rust
  fn create_detached_tmux_pane(
      backend: &dyn TmuxLayoutBackend,
      cmd: &str,
      cwd: &str,
      session_name: &str,
  ) -> Result<String, String> {
      let target_session = format!("{session_name}-{}-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(), std::process::id());
      prepare_detached_tmux_server(backend)?;
      backend.tmux_run(&["new-session", "-d", "-x", "160", "-y", "48", "-s", &target_session, "-c", cwd, "-F", "#{pane_id}", "bash", "-c", "sleep 365d"], true, false)
          .map_err(|e| format!("new-session failed: {e}"))?;
      let pane_id = backend.tmux_run(&["list-panes", "-t", &target_session, "-F", "#{pane_id}"], true, true)
          .map_err(|e| format!("list-panes failed: {e}"))?;
      let pane_id = pane_id.lines().next().unwrap_or("").trim().to_string();
      if pane_id.is_empty() {
          return Err(format!("failed to create detached tmux pane for session {target_session}"));
      }
      backend.tmux_run(&["respawn-pane", "-k", "-t", &pane_id, "-c", cwd, cmd], true, false)
          .map_err(|e| format!("respawn-pane failed: {e}"))?;
      Ok(pane_id)
  }

  fn prepare_detached_tmux_server(backend: &dyn TmuxLayoutBackend) -> Result<(), String> {
      let commands: Vec<Vec<&str>> = vec![
          vec!["start-server"],
          vec!["set-option", "-g", "destroy-unattached", "off"],
          vec!["set-option", "-g", "mouse", "on"],
          vec!["set-option", "-g", "history-limit", "50000"],
          vec!["set-option", "-g", "set-clipboard", "on"],
          vec!["set-option", "-g", "focus-events", "on"],
          vec!["set-option", "-g", "escape-time", "10"],
          vec!["set-window-option", "-g", "mode-keys", "vi"],
          vec!["bind-key", "-T", "copy-mode-vi", "v", "send-keys", "-X", "begin-selection"],
          vec!["bind-key", "-T", "copy-mode-vi", "C-v", "send-keys", "-X", "rectangle-toggle"],
          vec!["bind-key", "-T", "copy-mode-vi", "y", "send-keys", "-X", "copy-pipe-and-cancel", CLIPBOARD_PIPE_COMMAND],
          vec!["bind-key", "-T", "copy-mode-vi", "Enter", "send-keys", "-X", "copy-pipe-and-cancel", CLIPBOARD_PIPE_COMMAND],
          vec!["bind-key", "-T", "copy-mode-vi", "MouseDragEnd1Pane", "send-keys", "-X", "copy-pipe-and-cancel", CLIPBOARD_PIPE_COMMAND],
      ];
      for args in commands {
          let _ = backend.tmux_run(&args, false, false);
      }
      Ok(())
  }
  ```

  并定义常量：
  ```rust
  const CLIPBOARD_PIPE_COMMAND: &str = "sh -lc 'tmp=$(mktemp \"${TMPDIR:-/tmp}/ccb-clipboard.XXXXXX\") || exit 0; cat >\"$tmp\"; if command -v wl-copy >/dev/null 2>&1 && [ -n \"${WAYLAND_DISPLAY:-}\" ]; then (wl-copy <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v xclip >/dev/null 2>&1 && [ -n \"${DISPLAY:-}\" ]; then (xclip -selection clipboard <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v xsel >/dev/null 2>&1 && [ -n \"${DISPLAY:-}\" ]; then (xsel --clipboard --input <\"$tmp\"; rm -f \"$tmp\") >/dev/null 2>&1 & elif command -v pbcopy >/dev/null 2>&1; then pbcopy <\"$tmp\"; rm -f \"$tmp\"; else rm -f \"$tmp\"; fi'";
  ```

  为 `FakeBackend` 增加尺寸控制：
  ```rust
  fn pane_meets_minimum_size(&self, pane_id: &str, min_width: u32, min_height: u32) -> bool { ... }
  ```

- [ ] **Step 4: 运行新增测试确认通过**

  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-daemon test_pane_too_small_triggers_detached_fallback -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 5: 写失败测试 `test_namespace_launch_rejects_detached_fallback`**

  测试当 `allow_detached_fallback=false` 且 pane 尺寸不足时返回错误：

  ```rust
  #[test]
  fn test_namespace_launch_rejects_detached_fallback() {
      let tmp = tempfile::tempdir().unwrap();
      let root = tmp.path().to_string_lossy().to_string();
      let backend = Arc::new(FakeBackend::new("%99"));
      backend.mark_alive("%1", true);
      let mut impl_ = make_ensure_impl_with_min_size(backend.clone(), 80, 24);
      impl_ = impl_.with_allow_detached_fallback(false);

      let result = EnsureAgentRuntimeFn::call(
          &impl_,
          &Context { project_id: "proj".into(), project_root: root.clone(), workspace_path: root.clone() },
          &Command { restore: false },
          &AgentSpec { name: "agent1".into(), runtime_mode: "pane".into(), provider: "codex".into() },
          &Plan { workspace_path: root },
          None,
          None,
          0,
          Some("/tmp/tmux.sock"),
      );

      assert!(result.is_err());
      assert!(result.unwrap_err().contains("could not allocate stable tmux pane"));
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-daemon test_namespace_launch_rejects_detached_fallback -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 6: 写失败测试 `test_detached_fallback_when_no_space_for_new_pane`**

  让 `FakeBackend::create_pane` 返回错误信息 `split-window failed: no space for new pane`：

  ```rust
  #[test]
  fn test_detached_fallback_when_no_space_for_new_pane() {
      let tmp = tempfile::tempdir().unwrap();
      let root = tmp.path().to_string_lossy().to_string();
      let backend = Arc::new(FakeBackend::new("%99").with_create_pane_error("split-window failed: no space for new pane"));
      let impl_ = make_ensure_impl(backend.clone());

      let result = EnsureAgentRuntimeFn::call(
          &impl_,
          &Context { project_id: "proj".into(), project_root: root.clone(), workspace_path: root.clone() },
          &Command { restore: false },
          &AgentSpec { name: "agent1".into(), runtime_mode: "pane".into(), provider: "codex".into() },
          &Plan { workspace_path: root },
          None,
          None,
          0,
          Some("/tmp/tmux.sock"),
      )
      .unwrap();

      assert!(result.launched);
      assert_eq!(result.binding.unwrap().runtime_ref.as_deref(), Some("tmux:%99"));
      assert!(backend.has_call("tmux_run:new-session"));
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-daemon test_detached_fallback_when_no_space_for_new_pane -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 7: 提交**

  ```bash
  git add rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs
  git commit -m "feat(daemon): detached fallback + pane size + namespace limits for ensure_agent_runtime"
  ```

---

## Task 2: Runtime launch — foreign binding 检测与拒绝复用

**Files:**
- Modify: `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs`
- Modify: `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs`
- Modify: `rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs`
- Test: `rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`

- [ ] **Step 1: 扩展 `RuntimeBinding` 字段**

  在 `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs` 中给 `RuntimeBinding` 增加：

  ```rust
  pub provider_identity_state: Option<String>,
  pub provider_identity_reason: Option<String>,
  pub ccb_project_id: Option<String>,
  pub pane_state: Option<String>,
  ```

- [ ] **Step 2: 写失败测试 `test_foreign_binding_is_not_reused`**

  在测试文件中新增：

  ```rust
  #[test]
  fn test_foreign_binding_is_not_reused() {
      let tmp = tempfile::tempdir().unwrap();
      let root = tmp.path().to_string_lossy().to_string();
      let backend = Arc::new(FakeBackend::new("%99"));
      backend.mark_alive("%42", true);
      let impl_ = make_ensure_impl(backend.clone());

      let foreign_binding = RuntimeBinding {
          runtime_ref: Some("tmux:%42".into()),
          session_ref: Some("/tmp/proj/.ccb/.codex-agent1-session".into()),
          tmux_socket_path: Some("/tmp/other-project.sock".into()),
          ccb_project_id: Some("other-project".into()),
          pane_state: Some("foreign".into()),
          ..Default::default()
      };

      let result = EnsureAgentRuntimeFn::call(
          &impl_,
          &Context { project_id: "proj".into(), project_root: root.clone(), workspace_path: root.clone() },
          &Command { restore: false },
          &AgentSpec { name: "agent1".into(), runtime_mode: "pane".into(), provider: "codex".into() },
          &Plan { workspace_path: root },
          Some(&foreign_binding),
          None,
          0,
          Some("/tmp/proj.sock"),
      )
      .unwrap();

      assert!(result.launched);
      assert_eq!(result.binding.unwrap().runtime_ref.as_deref(), Some("tmux:%99"));
      assert!(backend.has_call("tmux_run:kill-pane"));
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-daemon test_foreign_binding_is_not_reused -- --test-threads=1
  ```
  期望：FAIL（foreign 检测未实现，导致复用旧 binding）。

- [ ] **Step 3: 在 `EnsureAgentRuntimeImpl::call` 中实现 foreign 检测**

  在 Step 1 的复用检查之前增加：

  ```rust
  fn binding_is_foreign(binding: &RuntimeBinding, project_id: &str, tmux_socket_path: Option<&str>) -> bool {
      if binding.pane_state.as_deref() == Some("foreign") {
          return true;
      }
      if let Some(binding_project) = binding.ccb_project_id.as_deref() {
          if binding_project != project_id {
              return true;
          }
      }
      if let (Some(a), Some(b)) = (binding.tmux_socket_path.as_deref(), tmux_socket_path) {
          if a != b {
              return true;
          }
      }
      false
  }
  ```

  在复用检查前调用：若 foreign，则跳过后续复用逻辑，进入 stale 清理+重新 launch。

- [ ] **Step 4: 调整 `compute_launch_binding_hint` 拒绝 foreign raw_binding**

  在 `rust/crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs` 的 `compute_launch_binding_hint` 中，当 `raw_binding` 存在且 `stale_binding=true` 时，若 raw_binding 是 foreign（socket 不匹配或 project_id 不匹配），返回 `None`。

- [ ] **Step 5: 运行测试确认通过**

  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-daemon test_foreign_binding_is_not_reused -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 6: 提交**

  ```bash
  git add rust/crates/ccb-daemon/src/start_runtime/agent_runtime_models.rs rust/crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs rust/crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs rust/crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs
  git commit -m "feat(daemon): detect and reject foreign runtime bindings"
  ```

---

## Task 3: Completion — `SessionRotate` selector reset

**Files:**
- Test: `rust/crates/ccb-completion/tests/integration_tests.rs`

- [ ] **Step 1: 写失败测试 `tracker_resets_selector_on_session_rotate`**

  在 `rust/crates/ccb-completion/tests/integration_tests.rs` 的 Tracker service tests 区域新增：

  ```rust
  #[test]
  fn tracker_resets_selector_on_session_rotate() {
      let manifest = manifest_for(
          CompletionFamily::ProtocolTurn,
          SelectorFamily::FinalMessage,
          CompletionSourceKind::ProtocolEventStream,
      );
      let resolver = resolver_for(manifest);
      let mut service = CompletionTrackerService::new(project_config(), resolver, CompletionRegistry);

      let job = JobRecord::new("job-1", "agent1", "claude");
      service.start(&job, ts()).unwrap();

      service.ingest(
          &job.job_id,
          &item_with_text(CompletionItemKind::TurnBoundary, "reply", "first"),
      ).unwrap();
      let view = service.current(&job.job_id).unwrap();
      assert_eq!(view.decision.reply, "first");

      service.ingest(
          &job.job_id,
          &CompletionItem {
              kind: CompletionItemKind::SessionRotate,
              timestamp: ts().to_string(),
              cursor: cursor(),
              provider: "claude".into(),
              agent_name: "agent1".into(),
              req_id: "job-1".into(),
              payload: Default::default(),
          },
      ).unwrap();

      let view_after_rotate = service.current(&job.job_id).unwrap();
      assert!(!view_after_rotate.decision.terminal);
      assert!(view_after_rotate.decision.reply.is_empty());
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-completion tracker_resets_selector_on_session_rotate -- --test-threads=1
  ```
  期望：若当前实现已 reset，则 PASS；若未 reset 则 FAIL（用于 TDD）。

- [ ] **Step 2: 确保 `CompletionTrackerService::ingest` 已 reset selector**

  当前 `rust/crates/ccb-completion/src/tracker.rs:172-174` 已有：
  ```rust
  if item.kind == CompletionItemKind::SessionRotate {
      tracker.selector.reset();
  }
  ```
  若 Step 1 失败，检查此处是否生效（`selector.reset()` 是否被 `FinalMessageSelector::reset` 正确清空）。

- [ ] **Step 3: 运行测试确认通过**

  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-completion tracker_resets_selector_on_session_rotate -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 4: 提交**

  ```bash
  git add rust/crates/ccb-completion/tests/integration_tests.rs
  git commit -m "test(completion): assert selector reset on SessionRotate"
  ```

---

## Task 4: Heartbeat — 清理 `classifier.rs` stub

**Files:**
- Modify: `rust/crates/ccb-heartbeat/src/classifier.rs`
- Test: `rust/crates/ccb-heartbeat/tests/integration.rs`

- [ ] **Step 1: 替换 `classifier.rs` 为空 stub 为 re-export 注释**

  将 `rust/crates/ccb-heartbeat/src/classifier.rs` 内容替换为：

  ```rust
  //! Python `lib/maintenance_heartbeat/classifier.py` functionality is
  //! implemented in `crate::maintenance`. This module re-exports the public
  //! classification symbols for callers that expect a `classifier` path.

  pub use crate::maintenance::{
      evaluate_project_view, evaluate_ps_summary, MaintenanceHeartbeatEvaluation,
      HEALTH_CONCERN, HEALTH_FAILING, HEALTH_HEALTHY, HEALTH_UNKNOWN,
      RECOMMENDED_ACTION_ASSESS_LATER, RECOMMENDED_ACTION_NONE,
  };
  ```

- [ ] **Step 2: 在 `lib.rs` 中导出 `classifier` 模块**

  确认 `rust/crates/ccb-heartbeat/src/lib.rs` 已有 `pub mod classifier;`。若没有则添加。

- [ ] **Step 3: 写测试 `classifier_reexport_smoke`**

  在 `rust/crates/ccb-heartbeat/tests/integration.rs` 末尾新增：

  ```rust
  #[test]
  fn classifier_reexport_smoke() {
      let evaluation = ccb_heartbeat::classifier::evaluate_project_view(&serde_json::json!({
          "view": {
              "ccbd": {"state": "mounted", "health": "healthy", "generation": 1},
              "agents": [],
              "comms": [],
          },
          "cache": {"generated_at": "2026-06-10T12:00:00Z"},
      }));
      assert_eq!(evaluation.health, "healthy");
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-heartbeat classifier_reexport_smoke -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 4: 确认无 `TODO: align` 残留**

  ```bash
  grep -n "TODO: align" rust/crates/ccb-heartbeat/src/classifier.rs || true
  ```
  期望：无输出。

- [ ] **Step 5: 提交**

  ```bash
  git add rust/crates/ccb-heartbeat/src/classifier.rs rust/crates/ccb-heartbeat/src/lib.rs rust/crates/ccb-heartbeat/tests/integration.rs
  git commit -m "refactor(heartbeat): replace classifier stub with maintenance re-export"
  ```

---

## Task 5: Jobs — 事件日志跳过非 `job_event` 记录

**Files:**
- Modify: `rust/crates/ccb-jobs/src/store.rs`
- Test: `rust/crates/ccb-jobs/tests/store_integration.rs`

- [ ] **Step 1: 写失败测试 `event_store_skips_non_job_event_records`**

  在 `rust/crates/ccb-jobs/tests/store_integration.rs` 末尾新增：

  ```rust
  #[test]
  fn event_store_skips_non_job_event_records() {
      let dir = TempDir::new().unwrap();
      let p = Utf8Path::from_path(dir.path()).unwrap();
      let layout = ccb_storage::paths::PathLayout::new(p);
      let store = JobEventStore::new(&layout);
      store.append(&JobEvent {
          event_id: "e1".into(),
          job_id: "job1".into(),
          agent_name: "claude".into(),
          target_kind: TargetKind::Agent,
          target_name: "claude".into(),
          event_type: "job_started".into(),
          payload: serde_json::Value::Object(Default::default()),
          timestamp: "2025-01-01T00:00:00Z".into(),
      }).unwrap();

      let path = layout.target_events_path("agent", "claude").unwrap();
      std::fs::OpenOptions::new().append(true).open(path.as_std_path()).unwrap()
          .write_all(
              serde_json::json!({
                  "schema_version": 2,
                  "record_type": "agent_event",
                  "event_type": "codex_memory_projection_ok",
                  "provider": "codex",
                  "agent_name": "claude",
              }).to_string().as_bytes()
          ).unwrap();
      std::fs::OpenOptions::new().append(true).open(path.as_std_path()).unwrap()
          .write_all(b"\n").unwrap();

      store.append(&JobEvent {
          event_id: "e2".into(),
          job_id: "job1".into(),
          agent_name: "claude".into(),
          target_kind: TargetKind::Agent,
          target_name: "claude".into(),
          event_type: "job_completed".into(),
          payload: serde_json::Value::Object(Default::default()),
          timestamp: "2025-01-01T00:00:01Z".into(),
      }).unwrap();

      let (line_no, events) = store.read_since("claude", 0);
      assert_eq!(line_no, 3);
      assert_eq!(events.len(), 2);
      assert_eq!(events[0].event_id, "e1");
      assert_eq!(events[1].event_id, "e2");
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-jobs event_store_skips_non_job_event_records -- --test-threads=1
  ```
  期望：若当前 `JsonlStore::read_since` 已过滤则 PASS，否则 FAIL。

- [ ] **Step 2: 在 `JobEventStore::read_since_target` 中过滤非 `job_event` 记录**

  在 `rust/crates/ccb-jobs/src/store.rs` 的 `read_since_target` 中，读取到的 `rows` 需要过滤掉 `record_type != "job_event"` 的行。实现方式：

  ```rust
  let events: Vec<JobEvent> = rows
      .into_iter()
      .filter(|row| row.get("record_type").and_then(|v| v.as_str()) == Some("job_event"))
      .collect();
  ```

  注意：`JsonlStore::read_since` 当前返回的是已反序列化的 `T`，这里 `T=JobEvent`。如果 `record_type` 不符导致反序列化失败，需要改为 `read_since::<serde_json::Value>` 再手动过滤和反序列化。在 `store.rs` 中：

  ```rust
  let Ok((line_no, rows)) = self.jsonl.read_since::<serde_json::Value>(&path, start_line) else {
      return (0, Vec::new());
  };
  let events: Vec<JobEvent> = rows
      .into_iter()
      .filter(|row| row.get("record_type").and_then(|v| v.as_str()) == Some("job_event"))
      .filter_map(|row| serde_json::from_value::<JobEvent>(row).ok())
      .collect();
  (line_no, events)
  ```

- [ ] **Step 3: 运行测试确认通过**

  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-jobs event_store_skips_non_job_event_records -- --test-threads=1
  ```
  期望：PASS。

- [ ] **Step 4: 提交**

  ```bash
  git add rust/crates/ccb-jobs/src/store.rs rust/crates/ccb-jobs/tests/store_integration.rs
  git commit -m "fix(jobs): skip non-job_event records in event log read"
  ```

---

## Task 6: CLI maintenance — 实现 service 层 `status/tick/schedule/runner`

**Files:**
- Modify: `rust/crates/ccb-cli/src/services/maintenance.rs`
- Modify: `rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs`
- Test: `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs`

- [ ] **Step 1: 实现 `maintenance_status` 并写测试**

  在 `rust/crates/ccb-cli/src/services/maintenance.rs` 中替换为：

  ```rust
  use crate::context::CliContext;
  use crate::services::DaemonClient;
  use serde_json::{json, Value};

  const DEFAULT_INTERVAL_S: u32 = 900;
  const DEFAULT_MIN_INTERVAL_S: u32 = 90;

  fn heartbeat_config(context: &CliContext) -> Option<(bool, String, u32, u32)> {
      let config_result = ccb_agents::config::load_project_config(&context.paths).ok()?;
      let heartbeat = config_result.config.maintenance_heartbeat.as_ref()?;
      Some((
          heartbeat.enabled && heartbeat.startup_ensure,
          heartbeat.assessor.clone(),
          heartbeat.interval_s.unwrap_or(DEFAULT_INTERVAL_S),
          heartbeat.min_interval_s.unwrap_or(DEFAULT_MIN_INTERVAL_S),
      ))
  }

  pub fn maintenance_status(context: &CliContext) -> Value {
      let (enabled, assessor, interval_s, min_interval_s) = heartbeat_config(context)
          .unwrap_or((false, String::new(), DEFAULT_INTERVAL_S, DEFAULT_MIN_INTERVAL_S));
      let project_id = context.project_id().to_string();
      let store = match ccb_heartbeat::MaintenanceHeartbeatStore::new(context.paths.clone(), &project_id) {
          Ok(s) => s,
          Err(_) => {
              return json!({"maintenance_status": "error", "error": "failed to open maintenance store"});
          }
      };

      let schedule = store.load_schedule().to_record();
      let last_status = store.load_status().to_record();
      let runner = store.load_runner().to_record();
      let assessor_present = if assessor.is_empty() {
          false
      } else {
          ccb_agents::config::load_project_config(&context.paths)
              .map(|r| r.config.agents.contains_key(&assessor))
              .unwrap_or(false)
      };

      json!({
          "maintenance_status": "ok",
          "enabled": enabled,
          "assessor": assessor,
          "assessor_present": assessor_present,
          "interval_s": interval_s,
          "min_interval_s": min_interval_s,
          "schedule": schedule,
          "last_status": last_status,
          "runner": runner,
      })
  }

  pub fn maintenance_tick(_context: &CliContext, _client: &dyn DaemonClient) -> Value {
      // Task 7 实现完整 tick 编排
      json!({"maintenance_status": "ok", "tick_status": "disabled"})
  }

  pub fn maintenance_schedule(_context: &CliContext, _after_s: u32, _reason: &str) -> Value {
      // Task 7 实现 schedule 写入
      json!({"maintenance_status": "ok", "schedule_status": "scheduled"})
  }

  pub fn maintenance_runner(_context: &CliContext, _client: &dyn DaemonClient, _max_iterations: u32) -> Value {
      // Task 7 实现 runner 循环
      json!({"maintenance_status": "ok", "runner_status": "stopped", "runner_exit_reason": "not_implemented"})
  }
  ```

  在 `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs` 中新增：

  ```rust
  #[test]
  fn test_cli_maintenance_status() {
      let dir = TempDir::new().unwrap();
      let (server, handle, _socket) = spawn_daemon(&dir);
      let project = dir.path().to_str().unwrap();
      std::fs::write(
          dir.path().join(".ccb/ccb.config"),
          "demo:codex\n\n[maintenance.heartbeat]\nenabled = true\nassessor = \"demo\"\ninterval_s = 900\nmin_interval_s = 90\nstartup_ensure = true\n",
      ).unwrap();

      let code = run(&["--project", project, "maintenance", "status"]);
      assert_eq!(code, 0);

      server.shutdown();
      handle.join().unwrap();
  }
  ```

  运行命令：
  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-cli test_cli_maintenance_status -- --test-threads=1
  ```
  期望：PASS（status 直接读本地 store/config，不依赖 daemon）。

- [ ] **Step 2: 扩展 `render_maintenance` 输出**

  在 `rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs` 的 `render_maintenance` 中，在现有字段后追加：

  ```rust
  if let Some(tick_status) = payload.get("tick_status").and_then(|v| v.as_str()) {
      lines.push(format!("tick_status: {}", tick_status));
  }
  if let Some(runner_status) = payload.get("runner_status").and_then(|v| v.as_str()) {
      lines.push(format!("runner_status: {}", runner_status));
  }
  if let Some(exit) = payload.get("runner_exit_reason").and_then(|v| v.as_str()) {
      lines.push(format!("runner_exit_reason: {}", exit));
  }
  if let Some(schedule_state) = payload.get("schedule_state").and_then(|v| v.as_str()) {
      lines.push(format!("schedule_state: {}", schedule_state));
  }
  if let Some(activation) = payload.get("tick_activation_status").and_then(|v| v.as_str()) {
      lines.push(format!("tick_activation_status: {}", activation));
  }
  ```

- [ ] **Step 3: 提交**

  ```bash
  git add rust/crates/ccb-cli/src/services/maintenance.rs rust/crates/ccb-cli/src/render_runtime/ops_views_basic.rs rust/crates/ccb-cli/tests/cli_maintenance_tests.rs
  git commit -m "feat(cli): maintenance status service + render + test"
  ```

---

## Task 7: CLI maintenance — 完整 `tick/schedule/runner` 编排

**Files:**
- Modify: `rust/crates/ccb-cli/src/services/maintenance.rs`
- Modify: `rust/crates/ccb-cli/src/commands.rs`
- Test: `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs`

- [ ] **Step 1: 实现 `maintenance_tick` 完整编排**

  在 `services/maintenance.rs` 中实现：

  1. 读取配置；若未启用返回 `tick_status: disabled`。
  2. 尝试 `client.call("project_view", json!({"schema_version": 1}))`。
  3. 若成功，用 `ccb_heartbeat::evaluate_project_view` 评估；若失败，捕获错误后用本地 ps 信息构造 fallback payload，调用 `ccb_heartbeat::evaluate_ps_summary`。
  4. 根据评估结果：
     - `health == healthy`：`tick_status: healthy`，`next_heartbeat_after_s = interval_s`，`schedule.next_run_at = now + interval_s`。
     - `health == concern/unknown/failing`：`tick_status` 对应，`next_heartbeat_after_s = min_interval_s`，写入 activation（若未 `--no-dispatch` 则通过 daemon `submit` RPC 提交 self-activation，body 包含 diagnostic JSON；否则 `suppressed_reason: dispatch_disabled`）。
  5. 用 `MaintenanceHeartbeatLock::try_acquire` 获取 lock；失败则返回 `tick_status: locked`。
  6. 写入 `status.json` 和 `schedule.json`。
  7. 返回 payload 包含 `tick_status`、`tick_recommended_action`、`tick_evidence`、`tick_activation_status`、`tick_activation_job_id`、`status_written`、`schedule_written`。

  关键函数签名：
  ```rust
  pub fn maintenance_tick(
      context: &CliContext,
      client: &dyn DaemonClient,
      force: bool,
      no_dispatch: bool,
      now: &str,
  ) -> Value;
  ```

- [ ] **Step 2: 实现 `maintenance_schedule`**

  ```rust
  pub fn maintenance_schedule(context: &CliContext, after_s: u32, reason: &str, now: &str) -> Value {
      let project_id = context.project_id().to_string();
      let store = ccb_heartbeat::MaintenanceHeartbeatStore::new(context.paths.clone(), &project_id).unwrap();
      let (_, _, _, min_interval_s) = heartbeat_config(context).unwrap_or((false, String::new(), DEFAULT_INTERVAL_S, DEFAULT_MIN_INTERVAL_S));
      let effective_after_s = after_s.max(min_interval_s);
      let next_run_at = ccb_heartbeat::time::plus_seconds(now, effective_after_s);
      let schedule = ccb_heartbeat::MaintenanceHeartbeatSchedule::new(
          project_id,
          Some(next_run_at),
          Some(reason.to_string()),
          Some(now.to_string()),
          Some("user".to_string()),
      ).unwrap();
      store.save_schedule(&schedule).unwrap();
      json!({
          "maintenance_status": "ok",
          "schedule_status": "scheduled",
          "requested_after_s": after_s,
          "scheduled_after_s": effective_after_s,
      })
  }
  ```

  若 `ccb_heartbeat::time::plus_seconds` 不存在，先在 `rust/crates/ccb-heartbeat/src/time.rs` 中实现：

  ```rust
  pub fn plus_seconds(base: &str, seconds: u32) -> String {
      let dt = parse_timestamp(base).unwrap_or_else(|| chrono::Utc::now().into());
      (dt + chrono::Duration::seconds(seconds as i64))
          .to_rfc3339()
  }
  ```

  然后在 `rust/crates/ccb-heartbeat/src/lib.rs` 中 `pub use time::{seconds_between, plus_seconds};`（若尚未导出）。

- [ ] **Step 3: 实现 `maintenance_runner`**

  ```rust
  pub fn maintenance_runner(
      context: &CliContext,
      client: &dyn DaemonClient,
      runner_id: &str,
      max_iterations: u32,
      sleep_cap_s: u64,
      no_dispatch: bool,
      now: &str,
  ) -> Value {
      let project_id = context.project_id().to_string();
      let store = ccb_heartbeat::MaintenanceHeartbeatStore::new(context.paths.clone(), &project_id).unwrap();
      // 保存 runner 状态为 running
      // 循环 max_iterations 次：
      //   - 读取 schedule；若未到 next_run_at 则 sleep 到 min(next_run_at, sleep_cap_s)
      //   - 调用 maintenance_tick(...)
      //   - 更新 runner.last_tick_at / last_tick_status
      // 保存 runner 状态为 stopped，exit_reason = max_iterations
      // 返回 runner summary
      json!({"maintenance_status": "ok", "runner_status": "stopped", "runner_exit_reason": "max_iterations", "runner_iterations": max_iterations})
  }
  ```

- [ ] **Step 4: 修改 `commands.rs` 的 `maintenance` 函数**

  将 `rust/crates/ccb-cli/src/commands.rs:311-325` 替换为：

  ```rust
  pub fn maintenance(client: &dyn DaemonClient, cmd: &ParsedMaintenance, context: &CliContext) -> Result<String, String> {
      let result = match cmd.action.as_str() {
          "status" => crate::services::maintenance::maintenance_status(context),
          "tick" => {
              let force = cmd.args.iter().any(|a| a == "--force");
              let no_dispatch = cmd.args.iter().any(|a| a == "--no-dispatch");
              let now = chrono::Utc::now().to_rfc3339();
              crate::services::maintenance::maintenance_tick(context, client, force, no_dispatch, &now)
          }
          "schedule" => {
              let after = parse_schedule_after(&cmd.args).unwrap_or(900);
              let reason = parse_schedule_reason(&cmd.args).unwrap_or("manual_schedule".to_string());
              let now = chrono::Utc::now().to_rfc3339();
              crate::services::maintenance::maintenance_schedule(context, after, &reason, &now)
          }
          "runner" => {
              let runner_id = parse_runner_id(&cmd.args).unwrap_or_else(|| format!("ccb-runner-{}", std::process::id()));
              let max_iterations = parse_max_iterations(&cmd.args).unwrap_or(0);
              let sleep_cap_s = parse_sleep_cap(&cmd.args).unwrap_or(300);
              let no_dispatch = cmd.args.iter().any(|a| a == "--no-dispatch");
              let now = chrono::Utc::now().to_rfc3339();
              crate::services::maintenance::maintenance_runner(context, client, &runner_id, max_iterations, sleep_cap_s, no_dispatch, &now)
          }
          other => return Err(format!("maintenance action '{}' not supported", other)),
      };
      Ok(render_maintenance(&result))
  }
  ```

  新增辅助解析函数（放在同一文件底部 `#[cfg(test)]` 外）：

  ```rust
  fn parse_schedule_after(args: &[String]) -> Option<u32> {
      args.windows(2).find(|w| w[0] == "--after").and_then(|w| w[1].parse::<u32>().ok())
  }
  fn parse_schedule_reason(args: &[String]) -> Option<String> {
      args.windows(2).find(|w| w[0] == "--reason").map(|w| w[1].clone())
  }
  fn parse_runner_id(args: &[String]) -> Option<String> {
      args.windows(2).find(|w| w[0] == "--runner-id").map(|w| w[1].clone())
  }
  fn parse_max_iterations(args: &[String]) -> Option<u32> {
      args.windows(2).find(|w| w[0] == "--max-iterations").and_then(|w| w[1].parse::<u32>().ok())
  }
  fn parse_sleep_cap(args: &[String]) -> Option<u64> {
      args.windows(2).find(|w| w[0] == "--sleep-cap").and_then(|w| w[1].trim_end_matches('s').parse::<u64>().ok())
  }
  ```

  注意：需要确认 `ParsedMaintenance` 的 `args` 字段存在（已在 `models_start.rs` 中定义）。

- [ ] **Step 5: 写 CLI maintenance 集成测试**

  在 `rust/crates/ccb-cli/tests/cli_maintenance_tests.rs` 中新增：

  ```rust
  #[test]
  fn test_cli_maintenance_tick_healthy_writes_status() {
      let dir = TempDir::new().unwrap();
      let (server, handle, _socket) = spawn_daemon(&dir);
      let project = dir.path().to_str().unwrap();
      std::fs::write(
          dir.path().join(".ccb/ccb.config"),
          "demo:codex\n\n[maintenance.heartbeat]\nenabled = true\nassessor = \"demo\"\ninterval_s = 900\nmin_interval_s = 90\nstartup_ensure = true\n",
      ).unwrap();

      let code = run(&["--project", project, "maintenance", "tick"]);
      assert_eq!(code, 0, "maintenance tick should succeed");

      let store = ccb_heartbeat::MaintenanceHeartbeatStore::new(
          ccb_storage::paths::PathLayout::new(project),
          &ccb_storage::paths::PathLayout::new(project).project_id().to_string(),
      ).unwrap();
      assert_eq!(store.load_status().state, ccb_heartbeat::store::ReadState::Ok);

      server.shutdown();
      handle.join().unwrap();
  }
  ```

  类似地增加 `test_cli_maintenance_schedule` 和 `test_cli_maintenance_runner_due_tick`。

- [ ] **Step 6: 运行 CLI maintenance 测试**

  ```bash
  cd /home/agnitum/ccb/rust && cargo test -p ccb-cli --test cli_maintenance -- --test-threads=1
  ```
  期望：全部 PASS。

- [ ] **Step 7: 提交**

  ```bash
  git add rust/crates/ccb-cli/src/services/maintenance.rs rust/crates/ccb-cli/src/commands.rs rust/crates/ccb-cli/tests/cli_maintenance_tests.rs
  git commit -m "feat(cli): full maintenance tick/schedule/runner orchestration"
  ```

---

## Task 8: Matrix 更新与全局验证

**Files:**
- Modify: `plans/rust-python-test-parity-matrix.md`

- [ ] **Step 1: 更新 parity matrix**

  在 `plans/rust-python-test-parity-matrix.md` 中：

  - `runtime_launch` 行 Notes 追加：
    - `test_detached_fallback_when_no_space_for_new_pane` → `crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs::test_detached_fallback_when_no_space_for_new_pane`
    - `test_namespace_launch_rejects_detached_fallback` → `crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs::test_namespace_launch_rejects_detached_fallback`
    - `test_pane_too_small_triggers_detached_fallback` → `crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs::test_pane_too_small_triggers_detached_fallback`
    - `test_foreign_binding_is_not_reused` → `crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs::test_foreign_binding_is_not_reused`
  - `heartbeat` 行 Notes 追加：
    - CLI maintenance orchestration parity: `crates/ccb-cli/tests/cli_maintenance_tests.rs` (`status`, `tick`, `schedule`, `runner`).
  - `completion` 行 Notes 追加：
    - `tracker_resets_selector_on_session_rotate` → `crates/ccb-completion/tests/integration_tests.rs`。
  - `jobs` 行（如存在）追加：
    - `event_store_skips_non_job_event_records` → `crates/ccb-jobs/tests/store_integration.rs`。

- [ ] **Step 2: 全局验证命令**

  ```bash
  cd /home/agnitum/ccb/rust && cargo check --workspace
  cargo test -p ccb-daemon -- --test-threads=1
  cargo test -p ccb-completion -- --test-threads=1
  cargo test -p ccb-jobs -- --test-threads=1
  cargo test -p ccb-heartbeat -- --test-threads=1
  cargo test -p ccb-cli -- --test-threads=1
  cargo clippy --workspace --all-targets
  cargo fmt --check
  ```

  期望：全部通过。

- [ ] **Step 3: 提交**

  ```bash
  git add plans/rust-python-test-parity-matrix.md
  git commit -m "docs: update parity matrix for wave 2 core parity"
  ```

---

## Self-review

- [ ] **Spec coverage:** 检查 PRD 中 A/B/C 三个 Scope 是否都有对应 Task：
  - A → Task 1、Task 2
  - B → Task 3、Task 4、Task 5
  - C → Task 6、Task 7
- [ ] **Placeholder scan:** 在 `implement.md` 中搜索 `TBD|TODO|implement later|fill in details|similar to Task`，确认无匹配。
- [ ] **Type consistency:**
  - `EnsureAgentRuntimeImpl` 的 builder 方法命名一致（`with_allow_detached_fallback`、`with_min_pane_size`）。
  - `RuntimeBinding` 新增字段在 `agent_runtime_models.rs`、`agent_runtime_binding.rs`、`ensure_agent_runtime.rs`、测试中使用相同名称。
  - `maintenance_tick/schedule/runner` 的 `now` 参数统一为 `&str`（RFC3339）。
  - `ParsedMaintenance` 的 `args` 字段在 `models_start.rs` 已定义为 `Vec<String>`。
- [ ] **Stop/escalation boundaries:** 本计划不涉及 ccbd socket protocol、mailbox kernel contract、tmux namespace/pane identity 的深层改动；仅扩展 `EnsureAgentRuntimeImpl` 的 fallback 行为和 CLI maintenance 编排。
