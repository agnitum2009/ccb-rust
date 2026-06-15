# Worker Test Verification

Date: 2026-06-07

Worker: worker

## Commands

- `pytest -q test/test_project_memory.py test/test_project_memory_filters.py test/test_project_memory_real_context.py test/test_provider_memory_external_context.py test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_v2_runtime_launch.py`
- `python -m py_compile lib/project_memory/filters.py lib/project_memory/policy.py lib/project_memory/sources.py lib/project_memory/types.py lib/project_memory/materializer.py lib/project_memory/template.py lib/project_memory/seed.py lib/provider_core/memory_projection.py lib/provider_backends/claude/launcher_runtime/home.py`
- `CCB_REAL_PROJECT_MEMORY_CHECK=1 CCB_REAL_TEST_PROJECT=/home/bfly/yunwei/test_ccb2 pytest -q test/test_provider_memory_external_context.py`
- `command -v ccb_test && ccb_test --version || true` from `/home/bfly/yunwei/test_ccb2`

## Result

- Source automated suite: `226 passed, 1 skipped`.
- Compile check: passed.
- Opt-in external real project check: failed against the existing
  `/home/bfly/yunwei/test_ccb2` runtime state.
- External `ccb_test`: `/tmp/ccb-v7.2.1-install-smoke/prefix/ccb_test`,
  release/stable `ccb v7.3.5 fb12192 2026-06-07`.

## External Failure Evidence

Existing generated Codex provider-state files still contain old duplicated ask
protocol:

- `agent1/provider-state/codex/home/AGENTS.md`: `command ask "$TARGET"` appears
  twice, `## CCB Runtime Coordination Rules` appears once, and
  `## Ask Communication` appears once.
- `archi/provider-state/codex/home/AGENTS.md`: same pattern.
- `/home/bfly/yunwei/test_ccb2/.ccb/ccb_memory.md` still contains old v4
  `## Ask Communication` text and shell fallback.

## Conclusion

The current source automated tests and compile checks pass. The external real
project check failure is stale generated runtime/provider-state evidence, not a
current-source automated test failure. Final external validation still requires
regenerating managed memory from the source-under-test runtime in an external
project, then rerunning the opt-in check.
