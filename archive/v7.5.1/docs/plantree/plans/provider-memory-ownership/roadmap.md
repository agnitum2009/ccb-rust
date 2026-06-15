# Provider Memory Ownership Roadmap

Date: 2026-06-07

## Done

- Identified that managed Claude already excludes provider-native project
  `CLAUDE.md` from the CCB-generated bundle, but the Claude contract still
  describes the older include behavior.
- Identified duplicated ask protocol text between renderer-owned
  `CCB Runtime Coordination Rules` and the default `.ccb/ccb_memory.md`
  template.
- Identified that Codex generated `CODEX_HOME/AGENTS.md` currently includes
  provider-native project `AGENTS.md`, which can duplicate Codex native project
  memory loading.
- Identified that OpenCode generated config merges a CCB memory bridge with
  user `opencode.json` instructions and needs an explicit source ownership
  policy before changing `AGENTS.md` inclusion.
- Chose source-ownership manifest governance over text-level deduplication in
  [decisions/001-source-ownership-not-text-dedup.md](decisions/001-source-ownership-not-text-dedup.md).
- Received an agent3 read-only plan review that confirmed ownership manifest is
  the main solution and identified implementation-readiness gaps in
  [history/agent3-review-2026-06-07.md](history/agent3-review-2026-06-07.md).
- Received a second agent3 reverse review that again found no better
  alternative than source ownership, but confirmed the plan is not
  implementation-ready until the Codex and OpenCode blockers below are closed.
- Aligned the phase-gated implementation plan in
  [topics/implementation-sequence.md](topics/implementation-sequence.md).
- Completed the first readiness audit pass:
  - OpenCode 1.16.2 natively discovers project `AGENTS.md` while also loading
    configured `instructions`, so CCB excludes project `AGENTS.md` from the
    generated runtime bundle.
  - Codex source-home `AGENTS.md` is classified as filtered provider user
    memory.
  - New Claude route-mode installs no longer write
    `~/.claude/rules/ccb-config.md`.
- Aligned Claude, Codex, OpenCode, and startup-supervision contract language
  with the source ownership policy.
- Added provider memory policy and provider-user-memory filters in code.
- Changed generated memory policy so Claude, Codex, and OpenCode exclude
  provider-native project memory from CCB bundles, while Gemini keeps existing
  behavior until audited.
- Simplified the default `.ccb/ccb_memory.md` template so new files no longer
  duplicate renderer-owned ask protocol text.
- Changed Claude route-mode install behavior so it no longer writes
  `~/.claude/rules/ccb-config.md`.
- Verified the first implementation with `pytest -q
  test/test_project_memory.py test/test_project_memory_filters.py
  test/test_provider_core_memory_projection.py test/test_provider_profiles.py
  test/test_provider_hook_settings.py test/test_v2_runtime_launch.py`
  on 2026-06-07: 220 passed.
- Added the expanded test matrix in
  [topics/test-matrix.md](topics/test-matrix.md), including a realistic
  temporary-project fixture and an opt-in external real project context check.
- Verified the expanded automated suite with `pytest -q
  test/test_project_memory.py test/test_project_memory_filters.py
  test/test_project_memory_real_context.py
  test/test_provider_memory_external_context.py
  test/test_provider_core_memory_projection.py test/test_provider_profiles.py
  test/test_provider_hook_settings.py test/test_v2_runtime_launch.py`
  on 2026-06-07: 226 passed, 1 skipped.
- Added seed-aware `.ccb/ccb_memory.md` upgrade for unedited generated old
  templates so existing seeded v4 shared memory can move to the v5 template
  without overwriting user-edited files.
- Ran the opt-in external context check against the existing
  `/home/bfly/yunwei/test_ccb2` state. It failed on a stale generated Codex
  `AGENTS.md` that still embeds old v4 shared memory; the local `ccb_test`
  there points to a release install, so source-runtime validation remains open.
- Extended old shared-memory migration to upgrade exact known generated legacy
  templates even when `memory.seed.json` has been removed.
- Fixed the tmux UI version-detection import cycle that blocked source `ccbd`
  keeper startup during external validation.
- Ran source-runtime validation from `/home/bfly/yunwei/test_ccb2` with
  `/home/bfly/yunwei/ccb_source/ccb_test`. Source start succeeded, regenerated
  managed memory, and the opt-in external context check passed.
- Ran a four-provider external layer matrix in
  `/home/bfly/yunwei/test_ccb_provider_memory_matrix`. Source start succeeded
  for Codex, Claude, OpenCode, and Gemini with provider-user, project, shared,
  and agent-private sentinel layers present; the opt-in matrix check passed.
- Completed agent3, reviewer1, and archi review. The archi HIGH finding was
  fixed by making Claude route-mode install/uninstall preserve unmarked
  `~/.claude/rules/ccb-config.md` files and remove only CCB-marked files.
- Verified the final relevant suite with `pytest -q
  test/test_install_source_dev_mode.py
  test/test_project_memory.py test/test_project_memory_filters.py
  test/test_project_memory_real_context.py
  test/test_provider_memory_external_context.py
  test/test_provider_memory_external_matrix.py
  test/test_provider_core_memory_projection.py test/test_provider_profiles.py
  test/test_provider_hook_settings.py test/test_v2_runtime_launch.py
  test/test_v2_tmux_ui.py` on 2026-06-07: 243 passed, 2 skipped.
- Ran a real four-provider validation in `/home/bfly/yunwei/test_ccb2` after
  authorizing worker1 to modify the external test project config and fixtures.
  Source `ccb_test` launched Codex, Claude, OpenCode, and Gemini healthy; the
  generated provider contexts matched the ownership policy and the opt-in
  external context check passed.

## In Progress

- Gemini project-memory ownership audit.

## Resolved Implementation Readiness Blockers

1. Codex project `AGENTS.md` exclusion is now part of provider policy and the
   Codex contract.
2. OpenCode project `AGENTS.md` policy is audited and aligned with the OpenCode
   contract.

## Next

1. Audit Gemini `contextFileName` project-memory ownership before changing its
   provider-native project memory policy.

## Deferred

- Provider-specific UI for inspecting memory source manifests.
- Automatic migration of user-edited `.ccb/ccb_memory.md` content.
- Semantic deduplication or model-assisted memory compression.
- Broader memory budgeting, summarization, or token accounting features.
