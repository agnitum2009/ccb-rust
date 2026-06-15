# Architecture Optimization Roadmap

Date: 2026-05-18

## Done

- Captured the Architec full baseline in
  [topics/architec-baseline-diagnosis.md](topics/architec-baseline-diagnosis.md).
- Reviewed the first hotspot code surfaces and mapped them to existing tests:
  provider profiles, provider hook settings, storage classification, and the
  GitHub release checker.
- Recorded the initial strategy in
  [decisions/001-prioritize-behavior-preserving-boundary-extraction.md](decisions/001-prioritize-behavior-preserving-boundary-extraction.md).
- Completed Phase 1 shared provider-home memory projection helper extraction
  for Codex, Claude, and Gemini. Agent3 approved the patch after Codex
  `external_migration = false` feature override restoration, and the focused
  gate passed with `175 passed`.
- Completed Phase 2 release checker split. Agent3 approved it with no blocking
  concerns after local release checker and focused gates passed.
- Completed Phase 3 storage classification cleanup governance:
  `install.ps1` and `watch_fallback.py` are explicitly kept active after
  reference searches, provider-home storage rules moved into
  `lib/storage_classification/provider_home.py`, and the storage/watch/install
  tests passed.
- Completed Phase 4 post-phase Architec comparison. Overall score moved from
  `50.11` to `51.83`; see
  [topics/post-phase-architec-results.md](topics/post-phase-architec-results.md).
- Completed the follow-up release checker GitHub split:
  `release_checker_github.py` now keeps GitHub orchestration while workflow
  polling moved to `release_checker_workflows.py` and release asset/published
  state handling moved to `release_checker_assets.py`.
- Completed post-review hotspot iterations:
  - `startup_update.py` is now a compatibility facade over state, refresh, and
    flow modules.
  - OpenCode projection event writing uses a shared low-level provider-core
    primitive while keeping OpenCode-specific config-merge semantics local.
  - Codex and Gemini share provider-core memory bundle materialization.
  - Claude binary cache routing is split into route-context and path-specific
    helper steps.
  - Final `archi .` score is `54.75` with structure `67.14` and governance/full
    `42.37`.

## In Progress

- None for this optimization pass.

## Next

- Future pass: consider a narrower Claude binary cache scan-helper cleanup
  only if new tests are added for unreadable, ignored-entry, and source-active
  edge cases.
- Future pass: review `lib/storage_classification/service.py` for another safe
  classifier extraction after the storage-boundary plan settles.
- Future pass: review `lib/ccbd/services/dispatcher_runtime/callbacks.py` only
  with ccbd lifecycle contract context; it is not a good opportunistic cleanup
  target.

## Deferred

- Broad archive movement from `.architec/architec-archive-candidates.json`.
  The semantic judge found strong false-positive signals for active documents.
- Moving `lib/provider_model_shortcuts.py` or `lib/release_artifacts.py` out of
  the package root. Architec marked them as review-only with no import pressure.
- Deleting fallback files. The cleanup inventory marks many `fallback_branch`
  candidates, but these sit on runtime and recovery paths and need caller
  evidence before removal.

## Phase Gates

Phase 1 is complete when:

- shared memory projection event/result code is owned by one helper module
  instead of being duplicated in Codex, Claude, and Gemini home materializers;
- provider-specific auth/config semantics stay in provider modules;
- OpenCode's extended memory/config merge projection path is either explicitly
  left unchanged or covered by a follow-up extraction plan;
- the provider-profile, provider-hook, Gemini launcher, and runtime-launch test
  slices pass, including `test/test_v2_runtime_launch.py`.

Phase 2 is complete when:

- `check_release_state.py` no longer owns all release, GitHub, workflow, and
  output responsibilities in one file;
- any multi-file split is proven compatible with `ccb-github` skill projection
  and active skill sync checks;
- the existing release checker tests pass without broad fixture rewrites.

Phase 3 is complete when:

- cleanup candidates are separated into keep/review/archive/delete classes with
  explicit safety checks;
- no active contract, README, changelog, runtime fallback, or operational
  installer is archived only because of heuristic text matches.

For this pass, the explicit classes are:

- keep_active: reviewed active surfaces in
  [decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md](decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md),
  plus previously identified active README/changelog/contract surfaces;
- review: all remaining heuristic cleanup and archive candidates;
- archive/delete: no actions in this pass.
