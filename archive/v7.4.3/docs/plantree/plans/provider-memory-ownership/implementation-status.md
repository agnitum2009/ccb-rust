# Provider Memory Ownership Implementation Status

Date: 2026-06-07

## Current Phase

First implementation, seed-aware shared-memory upgrade, automated validation
coverage, external source-runtime validation, and four-provider layered memory
matrix validation landed in the worktree. The final multi-agent review blocker
was fixed and revalidated, and the real `/home/bfly/yunwei/test_ccb2` project
has now been exercised as a four-provider runtime fixture.

## Last Landed

- Added provider memory ownership policy and provider-user-memory filtering.
- Applied the policy to generated memory source loading.
- Excluded provider-native project memory from Claude, Codex, and OpenCode CCB
  bundles.
- Kept Gemini provider-native project memory behavior unchanged pending audit.
- Removed duplicated ask protocol from the default `.ccb/ccb_memory.md`
  template for newly created files.
- Stopped new Claude route-mode installs from writing
  `~/.claude/rules/ccb-config.md`.
- Aligned Claude, Codex, OpenCode, and ccbd startup contracts.
- Added a realistic temporary-project context test that runs the real Claude,
  Codex, OpenCode, and Gemini memory materializers.
- Added an opt-in external project context check for inspecting generated
  provider-state/runtime memory files under a real test directory such as
  `/home/bfly/yunwei/test_ccb2`.
- Refined provider-user-memory filtering so block removal does not collapse
  user-authored paragraph spacing.
- Removed the remaining Claude direct provider-native exclusion flag so source
  ownership is decided by provider policy.
- Added seed-aware `.ccb/ccb_memory.md` upgrade for unedited generated old
  templates. User-edited shared memory is left untouched.
- Added exact known generated legacy-template upgrade for projects whose
  `memory.seed.json` was removed but whose `.ccb/ccb_memory.md` still exactly
  matches an old generated template.
- Fixed a source-runtime startup import cycle in tmux UI version detection that
  blocked `ccbd` keeper startup during external validation.
- Added an opt-in external provider memory matrix test that validates Codex,
  Claude, OpenCode, and Gemini generated context after source-runtime startup
  with provider-user, project, shared, and agent-private memory layers present.
- Fixed the archi review HIGH finding in `install.sh`: Claude route-mode install
  and uninstall now remove `~/.claude/rules/ccb-config.md` only when it carries
  a known CCB marker; unmarked user-authored rules files are preserved.
- Added installer regression coverage for marked and unmarked external Claude
  route rules files in both install and uninstall paths.
- Completed a worker1 real four-provider validation by temporarily modifying
  `/home/bfly/yunwei/test_ccb2` to include Codex, Claude, OpenCode, and Gemini
  agents plus provider-user, project-native, shared, and private memory
  sentinels.

## Last Verified

2026-06-07:

```bash
pytest -q test/test_install_source_dev_mode.py test/test_project_memory.py test/test_project_memory_filters.py test/test_project_memory_real_context.py test/test_provider_memory_external_context.py test/test_provider_memory_external_matrix.py test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_v2_runtime_launch.py test/test_v2_tmux_ui.py
```

Result: 243 passed, 2 skipped. The skipped tests are opt-in external project
context checks.

```bash
bash -n install.sh
git diff --check
python -m py_compile test/test_install_source_dev_mode.py
```

Result: passed.

```bash
python -m py_compile lib/project_memory/filters.py lib/project_memory/policy.py lib/project_memory/sources.py lib/project_memory/types.py lib/project_memory/materializer.py lib/project_memory/template.py lib/project_memory/seed.py lib/provider_core/memory_projection.py lib/provider_backends/claude/launcher_runtime/home.py lib/cli/services/tmux_ui_runtime/helpers.py lib/cli/services/tmux_ui.py lib/ccbd/keeper_main.py
```

Result: passed.

External observation:

```bash
CCB_REAL_PROJECT_MEMORY_CHECK=1 CCB_REAL_TEST_PROJECT=/home/bfly/yunwei/test_ccb2 pytest -q test/test_provider_memory_external_context.py
```

Result on the existing `/home/bfly/yunwei/test_ccb2` runtime state: failed
because the current generated Codex `AGENTS.md` still embeds the old
`.ccb/ccb_memory.md` v4 `## Ask Communication` section. The local `ccb_test`
binary in that directory points to a release install under
`/tmp/ccb-v7.2.1-install-smoke/prefix`, so this is recorded as a stale external
runtime-state finding rather than current-source runtime validation.

External source-runtime validation:

```bash
cd /home/bfly/yunwei/test_ccb2
/home/bfly/yunwei/ccb_source/ccb_test doctor
/home/bfly/yunwei/ccb_source/ccb_test
```

Result: source wrapper reported `install_mode: source`; after fixing the tmux UI
import cycle, source start succeeded with `start_status: ok`, `ccbd_started:
true`, and agents `agent1, archi`.

```bash
CCB_REAL_PROJECT_MEMORY_CHECK=1 CCB_REAL_TEST_PROJECT=/home/bfly/yunwei/test_ccb2 pytest -q test/test_provider_memory_external_context.py
```

Result after source-runtime regeneration: 1 passed.

External four-provider layered memory matrix:

```bash
cd /home/bfly/yunwei/test_ccb_provider_memory_matrix
HOME=/home/bfly/yunwei/test_ccb_provider_memory_matrix/source_home /home/bfly/yunwei/ccb_source/ccb_test
```

Result: source start succeeded with agents `codexer, clauder, opencoder,
geminier`.

```bash
CCB_PROVIDER_MEMORY_MATRIX_CHECK=1 CCB_PROVIDER_MEMORY_MATRIX_PROJECT=/home/bfly/yunwei/test_ccb_provider_memory_matrix pytest -q test/test_provider_memory_external_matrix.py
```

Result: 1 passed.

Worker independent verification:

- `worker` reran the automated suite: `226 passed, 1 skipped`.
- `worker` reran the compile check: passed.
- `worker` reran the opt-in external check and confirmed the same stale runtime
  state failure in `test_ccb2`.
- Full record:
  [history/worker-test-2026-06-07.md](history/worker-test-2026-06-07.md).

Worker1 real `test_ccb2` verification after clear:

- `worker1` cleared `/home/bfly/yunwei/test_ccb2` with source `ccb_test`, ran
  `doctor`, refreshed source runtime, and reran the opt-in external context
  check.
- Result: source runtime healthy, Codex generated context passed, and
  `test/test_provider_memory_external_context.py` returned `1 passed`.
- Coverage caveat: `/home/bfly/yunwei/test_ccb2` currently configures only
  Codex agents (`agent1`, `archi`), so this is a real `test_ccb2` validation but
  not a four-provider validation in that directory.
- Full record:
  [history/worker1-real-testccb2-review-2026-06-07.md](history/worker1-real-testccb2-review-2026-06-07.md).

Worker1 real `test_ccb2` four-provider verification:

- `worker1` backed up the original test config inside `test_ccb2`, then
  configured real Codex, Claude, OpenCode, and Gemini agents with shared,
  provider-user, project-native, and agent-private sentinels.
- Runtime commands were run from `/home/bfly/yunwei/test_ccb2` with
  `/home/bfly/yunwei/ccb_source/ccb_test` and
  `HOME=/home/bfly/yunwei/test_ccb2/source_home`.
- Result: Codex, Claude, OpenCode, and Gemini launched healthy; the opt-in
  external context check returned `1 passed`.
- Generated context evidence matched policy:
  - Codex excludes project `AGENTS.md` and includes shared/provider-user/private
    memory.
  - Claude excludes project `CLAUDE.md` and includes shared/provider-user/private
    memory.
  - OpenCode memory bridge excludes project `AGENTS.md`, while generated config
    keeps `["AGENTS.md", ".ccb/runtime/memory/opencoder.md"]`.
  - Gemini keeps project `GEMINI.md` under the current pending-audit policy.
- Full record:
  [history/worker1-real-testccb2-four-provider-2026-06-07.md](history/worker1-real-testccb2-four-provider-2026-06-07.md).

## Active TODO

1. Audit Gemini `contextFileName` project-memory ownership before changing
   Gemini policy.

## Blocked By

- No current unit-test blocker.
- Runtime validation must not be run from `ccb_source`; use an external test
  project.
