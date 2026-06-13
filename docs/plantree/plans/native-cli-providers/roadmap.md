# Native CLI Providers Roadmap

Date: 2026-06-13

## Status Summary

- Current status: native completion pivot is in the working tree. Kimi,
  DeepSeek/DeepCode, and AGY no longer use `CCB_DONE` as their primary
  completion signal.
- Last verified: focused native completion tests, provider catalog tests, and
  stub-backed source-runtime asks for Kimi, DeepSeek, and AGY passed.
- Next target: review/commit.

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

## In Progress

- Broader review/release-gate validation if requested.

## Next

1. Review the smoke project artifacts under
   `/home/bfly/yunwei/test_ccb2/native_provider_smoke` and real Kimi project
   `/home/bfly/yunwei/test_ccb2/kimi_ccb_real`; keep or remove them before
   final cleanup.
2. Commit after user approval or review pass.

## Deferred

- Kimi prompt-mode adapter using `kimi --prompt` and `--output-format`.
- Provider-specific auth/config diagnostics for Kimi login and Deep Code
  `settings.json`.
- Support aliases such as `deepcode` if real user configs show that provider
  key is needed.
- Model/key/url shortcut projection after upstream config semantics are stable
  and tested.
