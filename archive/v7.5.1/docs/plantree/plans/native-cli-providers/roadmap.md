# Native CLI Providers Roadmap

Date: 2026-06-13

## Status Summary

- Current status: native completion pivot has landed in source. Kimi,
  DeepSeek/DeepCode, AGY, and MiMo no longer use `CCB_DONE` as their primary
  completion signal. Kimi and OpenCode inherited ask skill injection landed in
  commit `a4395c2`; MiMo inherited ask instruction injection and native
  `mimo run --format json` execution landed in commit `fce17c3`.
- Last verified: focused native completion tests, provider catalog tests,
  Kimi/OpenCode skill projection tests, and a real MiMo CCB ask passed after
  switching CCB MiMo execution to `mimo run --pure --format json`; full
  pytest release gate passed with `2613 passed, 2 skipped`.
- Next target: publish the 7.5 patch release that includes MiMo in the public
  README/provider surface.

## Done

- Confirmed Kimi Code is a terminal AI coding agent from Moonshot AI, launched
  with `kimi`, and npm package `@moonshot-ai/kimi-code` exposes bin `kimi`.
- Confirmed DeepSeek API docs list Deep Code as a terminal AI coding assistant
  for DeepSeek-V4, installed by `npm install -g @vegamo/deepcode-cli` and
  launched with `deepcode`.
- Chose provider keys:
  - `kimi` maps to executable `kimi`.
  - `deepseek` maps to executable `deepcode`.
- Chose first-slice completion strategy: pane-backed prompt wrapping and
  pane-text detection with `CCB_REQ_ID` and `CCB_DONE`, mirroring the earlier
  `agy` boundary while sharing new generic support code. This is now retained
  only as historical first-slice evidence and compatibility helper coverage.
- Added shared `pane_quiet_support` helpers for prompt wrapping, pane snapshots,
  start/poll behavior, done marker completion, empty-reply diagnostics, and
  input-unresponsive timeout.
- Added Kimi and DeepSeek backend modules with manifests, execution adapters,
  session bindings, and simple-tmux launchers.
- Registered both providers as optional built-ins, runtime specs, command
  defaults, session filenames, doctor/kill paths, tests, and config contract
  documentation.
- Validated and fixed real Kimi 1.47.0 behavior:
  - Kimi launcher does not append implicit `--continue`, because first launch
    without a workdir session exits in Kimi 1.47.0.
  - Pane-quiet parsing ignores single prompt-echo done markers and waits for
    the model's own done marker.
  - Pane-quiet parsing strips Kimi TUI's leading assistant bullet from the
    first reply line.
  - Kimi prompt delivery is deferred until the TUI input area is visible, so
    asks submitted immediately after start/restart are not lost before Kimi is
    ready.
- Validated source runtime with a stub-backed smoke project:
  - `config validate` accepted `kimi1:kimi, deep1:deepseek`.
  - `ccb_test -s` launched both providers through tmux.
  - `ccb_test ask` completed for both providers with
    `completion_reason: pane_done_marker`.
  - `ccb_test restart kimi1` and `ccb_test restart deep1` succeeded.
  - post-restart asks completed for both providers.
  - `reload --dry-run` returned `plan_class: no_change`.
- Validated source runtime with a real Kimi project:
  - `config validate` accepted `kimi1:kimi, kimi2:kimi`.
  - `ccb_test -s` launched both Kimi panes.
  - immediate post-start ask completed with exact reply
    `KIMI_READY_SEND_OK`.
  - serial ask set completed for `KIMI_SERIAL_OK_1..5`.
  - 8-job concurrent pressure set across both agents completed with exact
    replies `KIMI_AFTERFIX_OK_1..8`.
  - `restart kimi1`, post-restart ask, `clear kimi1 kimi2`, post-clear ask,
    artifact reply, `reload --dry-run`, and `ping` checks passed.
- Focused related pytest set after real-Kimi fixes: `113 passed`.
- Replaced primary completion detection for Kimi, DeepSeek/DeepCode, and AGY:
  - Kimi polls Kimi `wire.jsonl`, binds by `CCB_REQ_ID`, emits completion on
    native `TurnEnd`, and diagnoses `TurnEnd` with empty reply as
    `kimi_native_empty_reply`.
  - DeepSeek polls DeepCode `sessions-index.json` plus session jsonl, emits
    completion on native `status=completed`, and diagnoses completed empty
    replies as `deepseek_native_empty_reply`.
  - AGY polls Antigravity transcript logs and emits completion from native
    model `*_RESPONSE` events.
  - OpenCode already uses native storage and remains unchanged.
- Updated deterministic provider stubs to write Kimi, DeepCode, and AGY native
  stores instead of pane `CCB_DONE` for these providers.
- Focused native completion verification:
  `python -m pytest -q test/test_agy_execution_polling.py
  test/test_native_cli_completion.py
  test/test_native_cli_providers.py test/test_v2_provider_catalog.py
  test/test_pane_quiet_support.py`: `25 passed`.
- Explored upstream/source and local runtime evidence for native completion
  markers:
  - Kimi real 1.47.0 writes `TurnBegin`/`ContentPart`/`TurnEnd`; npm source
    also exposes `turn.started`/`assistant.delta`/`turn.ended`, now accepted as
    a compatibility input when present in a wire log.
  - DeepCode source confirms `permission_denied`; CCB now returns
    `deepseek_native_permission_denied` with diagnostics instead of waiting for
    timeout.
  - AGY local transcript inventory confirms `USER_EXPLICIT/USER_INPUT/DONE`
    plus `MODEL/*_RESPONSE/DONE` as the practical native completion marker.
- Added inherited ask skill projection for additional providers:
  - Kimi receives `inherit_skills/kimi_skills/ask/SKILL.md` through managed
    provider-state skills and `--skills-dir`; existing Kimi default project/user
    skill directories are preserved when CCB switches Kimi into explicit
    `--skills-dir` mode.
  - OpenCode receives `inherit_skills/opencode_skills/ask.md` through generated
    `.ccb/runtime/skills/<agent>/opencode/ask.md` and
    `opencode.json.instructions`.
  - OpenCode `inherit_memory` and `inherit_skills` are independent: memory can
    be disabled while keeping the ask instruction bridge.
  - OpenCode projection event de-duplication now includes skill hash evidence,
    so skill-only injection does not emit repeated unchanged events.
- Focused ask skill injection verification:
  `python -m pytest -q test/test_native_cli_providers.py
  test/test_provider_hook_settings.py test/test_v2_runtime_launch.py
  test/test_project_memory_real_context.py
  test/test_provider_memory_external_matrix.py test/test_storage_classification.py
  test/test_repo_hygiene.py test/test_ask_skill_templates.py`:
  `141 passed, 1 skipped`; `git diff --check` passed.
- Added MiMo Code as optional provider `mimo`:
  - Official package `@mimo-ai/cli@0.1.0` exposes binary `mimo`; local install
    reports `mimo --version` as `0.1.0`.
  - CCB startup still mounts a managed visible MiMo pane and materializes
    MiMo `mimocode.json` with memory plus ask instruction paths.
  - CCB ask execution uses a per-job native subprocess:
    `mimo run --pure --format json --dir <workdir> <wrapped prompt>`.
  - Completion is observed from JSON result events: `part.text` supplies the
    assistant reply and `step_finish` / `part.reason=stop` terminalizes with
    `completion_reason: mimo_run_stop`.
  - Completed-native-empty MiMo results terminalize as
    `mimo_run_empty_reply` instead of waiting for reliability timeout.
  - CCB passes `--pure` for MiMo run-mode asks so external plugin/tool-call
    intermediate steps do not consume simple CCB ask jobs before final text.
- MiMo verification:
  - Real installed `mimo run --format json --dir
    /home/bfly/yunwei/test_ccb2/mimo_real` completed with exact reply
    `MIMO_CCB_REAL_OK`.
  - Source-runtime real CCB project
    `/home/bfly/yunwei/test_ccb2/mimo_ccb_real` accepted `cmd; mimo1:mimo`,
    launched with `/home/bfly/yunwei/ccb_source/ccb_test -s`, and completed
    job `job_ae41cad0e98a` with reply `MIMO_CCB_RUN_OK_3` and
    `completion_reason: mimo_run_stop`.
  - Release-gate rerun with `--pure` completed job `job_023d114681ca` with
    reply `MIMO_RELEASE_751_OK` and `completion_reason: mimo_run_stop`; the
    preceding non-pure probe exposed `mimo_run_finished:tool-calls`, now
    covered as an intermediate finish reason.
  - Focused touched-provider tests:
    `python -m pytest -q test/test_mimo_provider.py
    test/test_native_cli_providers.py test/test_v2_provider_catalog.py
    test/test_v2_provider_core_registry.py test/test_runtime_specs.py
    test/test_v2_config_loader.py test/test_v2_runtime_launch.py
    test/test_storage_classification.py test/test_repo_hygiene.py
    test/test_ask_skill_templates.py test/test_provider_hook_settings.py
    test/test_project_memory_real_context.py
    test/test_provider_memory_external_matrix.py test/test_opencode_comm_sqlite.py
    test/test_opencode_execution_polling.py
    test/test_provider_execution_service_runtime.py`: `263 passed, 1 skipped`.
  - Final full release-gate pytest: `2613 passed, 2 skipped`.
  - `git diff --check`: passed.

## In Progress

- 7.5 patch release packaging for the landed MiMo provider integration.
- README/npm release surface now needs final gate checks and publication.

## Next

1. Publish `v7.5.1` through the release workflow/push-agent path.
2. Review reusable smoke projects under `/home/bfly/yunwei/test_ccb2` before
   final cleanup if the release gate requires a clean test directory.

## Deferred

- Kimi prompt-mode adapter using `kimi --prompt` and `--output-format`.
- Provider-specific auth/config diagnostics for Kimi login and Deep Code
  `settings.json`.
- MiMo provider-specific auth/config diagnostics if users hit
  `mimo run` account or model setup failures.
- Support aliases such as `deepcode` if real user configs show that provider
  key is needed.
- Model/key/url shortcut projection after upstream config semantics are stable
  and tested.
