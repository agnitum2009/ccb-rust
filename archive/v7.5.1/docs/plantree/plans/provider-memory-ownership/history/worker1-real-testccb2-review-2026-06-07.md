# Worker1 Real `test_ccb2` Review

Date: 2026-06-07

## Scope

`worker1` was cleared first, then asked to validate current source changes from
the real external project `/home/bfly/yunwei/test_ccb2` using
`/home/bfly/yunwei/ccb_source/ccb_test`.

## Commands Reported

- cwd `/home/bfly/yunwei/ccb_source`: confirmed `test_ccb2` is outside
  `ccb_source`.
- cwd `/home/bfly/yunwei/test_ccb2`:
  `/home/bfly/yunwei/ccb_source/ccb_test clear`
- cwd `/home/bfly/yunwei/test_ccb2`:
  `/home/bfly/yunwei/ccb_source/ccb_test doctor`
- cwd `/home/bfly/yunwei/test_ccb2`:
  `/home/bfly/yunwei/ccb_source/ccb_test`
- cwd `/home/bfly/yunwei/ccb_source`:
  `CCB_REAL_PROJECT_MEMORY_CHECK=1 CCB_REAL_TEST_PROJECT=/home/bfly/yunwei/test_ccb2 pytest -q test/test_provider_memory_external_context.py`
- cwd `/home/bfly/yunwei/ccb_source`:
  `pytest -q test/test_project_memory.py test/test_project_memory_filters.py test/test_provider_core_memory_projection.py test/test_install_source_dev_mode.py`

## Result

- `ccb_test clear`: passed, clearing `agent1` and `archi`.
- `ccb_test doctor`: passed, reporting source install
  `/home/bfly/yunwei/ccb_source`, `install_mode=source`, healthy mounted backend,
  and healthy Codex agents.
- `ccb_test`: passed with `start_status: ok`.
- opt-in external context check: passed, `1 passed`.
- focused source tests: passed, `39 passed`.

## Evidence

Generated Codex bundles:

- `/home/bfly/yunwei/test_ccb2/.ccb/agents/agent1/provider-state/codex/home/AGENTS.md`
  - `CCB Runtime Coordination Rules`: 1
  - `CCB Shared Project Memory`: 1
  - `Provider-Native Project Memory`: 0
  - project `AGENTS.md` sections such as `Startup And Backend Anchor` and
    `Non-Drift Rules`: 0
- `/home/bfly/yunwei/test_ccb2/.ccb/agents/archi/provider-state/codex/home/AGENTS.md`
  - `CCB Runtime Coordination Rules`: 1
  - `CCB Shared Project Memory`: 1
  - `Provider-Native Project Memory`: 0
  - project `AGENTS.md` sections such as `Startup And Backend Anchor` and
    `Non-Drift Rules`: 0
  - role memory present: `Role Memory: agentroles.archi`

Projection metadata for `agent1` and `archi` reported `status: ok`,
`reason: written`, and no warnings.

## Coverage Gap

This real project currently configures only Codex agents (`agent1` and
`archi`). It has no generated Claude, OpenCode, or Gemini provider-state/runtime
directories, and no root `AGENTS.md`/`CLAUDE.md`/`GEMINI.md` native project
memory files. Therefore this is a real `test_ccb2` source-runtime validation,
but not a complete four-provider real-context validation.

The earlier external provider matrix remains the four-provider coverage source.
To make `test_ccb2` itself fully cover all providers, its real configuration
must be changed to include Claude, OpenCode, and Gemini agents plus provider
native project memory fixtures, then regenerated with source `ccb_test`.

## Review Finding

HIGH: none.

MEDIUM: `test_ccb2` coverage is Codex-only, so it cannot prove Claude,
OpenCode, or Gemini behavior in that specific directory.

LOW: `test/test_provider_memory_external_context.py` passes when at least one
managed bundle exists; stricter all-provider coverage lives in the separate
provider matrix opt-in test.
