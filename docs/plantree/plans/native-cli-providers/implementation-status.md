# Native CLI Providers Implementation Status

Date: 2026-06-13

## Current Phase

Native completion pivot is implemented in the working tree. Kimi,
DeepSeek/DeepCode, and AGY now use provider-native session/event logs for
completion detection instead of asking the model to print `CCB_DONE`.

## Last Landed

- Shared pane-quiet support and Kimi/DeepSeek provider backends were added in
  the earlier first slice and remain as compatibility/test support.
- Source-runtime smoke project added at
  `/home/bfly/yunwei/test_ccb2/native_provider_smoke`.
- Real Kimi validation project added at
  `/home/bfly/yunwei/test_ccb2/kimi_ccb_real` with `kimi1:kimi, kimi2:kimi`.
- Kimi launcher no longer injects implicit `--continue`; Kimi 1.47.0 exits
  when no previous session exists for the workdir.
- Pane-quiet parsing now ignores prompt-echo done markers, strips Kimi TUI's
  leading assistant bullet, and defers Kimi prompt delivery until the TUI input
  area is ready.
- Current native pivot:
  - Kimi reads `~/.kimi/sessions/<project-md5>/<session>/wire.jsonl`.
  - DeepSeek reads `~/.deepcode/projects/<project-code>/sessions-index.json`
    plus `<session>.jsonl`.
  - AGY reads
    `~/.gemini/antigravity-cli/brain/<conversation>/.system_generated/logs/transcript*.jsonl`.
  - Provider stubs now write those native stores for source runtime tests.

## Active TODO

1. Decide whether to keep the smoke/real test projects as reusable validation
   fixtures.
2. Commit after review/approval.

## Blocked By

None for the first slice. Real provider API execution may require user-owned
Kimi/DeepSeek credentials; CCB integration can still be validated with
provider command templates and installed CLI help/version checks.

## Last Verified

Historical first-slice verification:

- `node --version` returned `v22.20.0`.
- `npm view @moonshot-ai/kimi-code@0.14.2 version bin engines --json` returned
  bin `kimi` and engine `>=22.19.0`.
- `npm view @vegamo/deepcode-cli@0.1.29 version bin engines --json` returned
  bin `deepcode` and engine `>=22`.
- `npx --yes @moonshot-ai/kimi-code@0.14.2 --help` and `--version` succeeded.
- `npx --yes @vegamo/deepcode-cli@0.1.29 --help` and `--version` succeeded.
- `python -m pytest -q test/test_pane_quiet_support.py
  test/test_native_cli_providers.py test/test_v2_provider_catalog.py
  test/test_v2_provider_core_registry.py test/test_runtime_specs.py`:
  `18 passed`.
- Focused config/runtime-launch checks: `3 passed`.
- Full `test/test_v2_config_loader.py`: `87 passed`.
- Full `test/test_v2_runtime_launch.py`: `78 passed`.
- Kill/provider catalog/registry focused set: `12 passed`.
- Execution/completion/session-binding related set: `88 passed`.
- `python -m py_compile` for new provider modules and touched stubs passed.
- `git diff --check` passed.
- Full repository `python -m pytest -q`: `2585 passed, 2 skipped`.
- `/home/bfly/yunwei/ccb_source/ccb_test --diagnose` passed from
  `/home/bfly/yunwei/test_ccb2/native_provider_smoke`.
- Smoke `ccb_test config validate`: valid, agents `deep1, kimi1`.
- Smoke `ccb_test -s`: launched `kimi1` and `deep1`.
- Smoke `ccb_test ask`: both providers completed with
  `completion_reason: pane_done_marker`.
- Smoke `ccb_test restart kimi1` and `restart deep1`: both `restart_status: ok`.
- Smoke post-restart asks: both completed with `pane_done_marker`.
- Smoke `ccb_test reload --dry-run`: `plan_class: no_change`.
- Smoke runtime stopped with `ccb_test kill -f`.
- Real Kimi CLI install check from `/home/bfly/yunwei/test_ccb2`:
  `kimi --version` returned `kimi, version 1.47.0`.
- Real Kimi CCB project
  `/home/bfly/yunwei/test_ccb2/kimi_ccb_real`:
  `ccb_test config validate` valid, agents `kimi1, kimi2`.
- Real Kimi `ccb_test -s` launched both Kimi panes with command `kimi`.
- Real Kimi immediate post-start ask completed with
  `reply: KIMI_READY_SEND_OK` and `completion_reason: pane_done_marker`,
  proving ready-before-send protection.
- Real Kimi serial ask set completed for `KIMI_SERIAL_OK_1` through
  `KIMI_SERIAL_OK_5`.
- Real Kimi after-fix concurrent pressure set submitted 8 jobs across
  `kimi1` and `kimi2`; all completed with exact replies
  `KIMI_AFTERFIX_OK_1` through `KIMI_AFTERFIX_OK_8`.
- Real Kimi `ccb_test restart kimi1` succeeded; post-restart ask completed
  with `reply: KIMI_RESTART_AFTERFIX_OK`.
- Real Kimi `ccb_test clear kimi1 kimi2` succeeded; post-clear ask completed
  with `completion_reason: pane_done_marker`.
- Real Kimi artifact reply path stored
  `job_29c1cf2fb1a2-art_0bc032960b444864.txt`.
- Focused related pytest set after the real-Kimi fixes:
  `113 passed`.

Current native pivot verification:

- Native pivot compile check:
  `python -m py_compile test/stubs/provider_stub.py
  lib/provider_backends/kimi/execution.py
  lib/provider_backends/deepseek/execution.py
  lib/provider_backends/agy/execution_runtime/poll.py`: passed.
- Native pivot focused tests:
  `python -m pytest -q test/test_agy_execution_polling.py
  test/test_native_cli_completion.py
  test/test_native_cli_providers.py test/test_v2_provider_catalog.py
  test/test_pane_quiet_support.py`: `25 passed`.
- Native pivot focused config/catalog tests:
  `python -m pytest -q test/test_native_cli_completion.py
  test/test_native_cli_providers.py test/test_v2_provider_catalog.py
  test/test_v2_provider_core_registry.py test/test_runtime_specs.py
  test/test_v2_config_loader.py -k 'native or provider_catalog or optional or
  kimi or deepseek or agy or runtime_spec'`: `16 passed, 90 deselected`.
- Source-runtime smoke from
  `/home/bfly/yunwei/test_ccb2/native_provider_smoke` with isolated
  `HOME=/home/bfly/yunwei/test_ccb2/source_home`:
  - `ccb_test config validate`: valid, agents `agy1, deep1, kimi1`.
  - `ccb_test -s`: `start_status: ok`, agents `kimi1, deep1, agy1`.
  - `ccb_test ask kimi1`: job `job_462bb2fd5afb`, reply completed with
    reason `kimi_turn_end`.
  - `ccb_test ask deep1`: job `job_bfa7f505da0f`, reply completed with
    reason `deepseek_session_completed`.
  - `ccb_test ask agy1`: job `job_bd583f5b76cb`, reply completed with
    reason `agy_transcript_response_done`.
  - Native files observed under source home:
    Kimi `wire.jsonl`, DeepCode `sessions-index.json`/session jsonl, and AGY
    `transcript.jsonl`.
  - `ccb_test ping all`: mounted and idle for `agy1`, `deep1`, `kimi1`.
  - Smoke runtime stopped with `ccb_test kill -f`.
- Source-marker probe:
  - Kimi real 1.47.0 local wire logs use
    `TurnBegin`/`ContentPart`/`StatusUpdate`/`TurnEnd`.
  - Kimi npm source `@moonshot-ai/kimi-code@0.14.2` also exposes event-stream
    names `turn.started`/`assistant.delta`/`turn.ended`; the Kimi parser now
    accepts source-style `turn.prompt`/`assistant.delta`/`turn.ended` records
    when they appear in `wire.jsonl`.
  - Kimi launcher now injects `--auto-approve` for CCB auto-permission on
    current Kimi versions, while treating legacy/alias flags
    `--auto`, `--auto-approve`, `--yes`, `-y`, and `--yolo` as explicit
    auto-permission flags to avoid duplication.
  - DeepCode source `@vegamo/deepcode-cli@0.1.29` confirms
    `permission_denied`; DeepSeek polling now terminalizes it as
    `deepseek_native_permission_denied`.
  - AGY local runtime artifacts confirm transcript `DONE` rows as the stable
    evidence surface; sqlite conversation DB status enums remain diagnostic
    only.
  - OpenCode npm package `opencode-ai@1.16.2` is a binary installer wrapper;
    CCB's existing OpenCode storage contract remains the native completion
    authority (`time.completed`).
- Source-marker focused verification:
  - `python -m py_compile lib/provider_backends/kimi/native_log.py
    lib/provider_backends/deepseek/native_log.py
    lib/provider_backends/deepseek/execution.py
    test/test_native_cli_completion.py`: passed.
  - `python -m pytest -q test/test_native_cli_completion.py`: `8 passed`.
  - `python -m pytest -q test/test_agy_execution_polling.py
    test/test_native_cli_completion.py test/test_native_cli_providers.py
    test/test_v2_provider_catalog.py test/test_pane_quiet_support.py`:
    `27 passed`.
- Kimi auto flag compatibility verification:
  - `kimi --auto-approve --version`: succeeded on local Kimi 1.47.0.
  - `python -m py_compile lib/provider_backends/kimi/launcher.py
    test/test_native_cli_providers.py`: passed.
  - `python -m pytest -q test/test_native_cli_providers.py`: `5 passed`.
