# Implementation Status

Date: 2026-05-18

## Current Phase

Phase 1 through Phase 4 are complete, reviewed, and verified.
The follow-up release checker GitHub split and post-review hotspot iterations are
implemented and locally verified.

## Active TODO

- None. The current pass has reached diminishing returns; remaining hotspots
  are distributed across several runtime/tooling files instead of one dominant
  giant module.

## Done This Phase

- Ran `archi .` and captured baseline artifacts under `.architec/`.
- Inspected provider materialization code, storage classification code, release
  checker code, and existing tests.
- Created this planning tree.
- Added `lib/provider_core/memory_projection.py`.
- Updated Codex, Claude, and Gemini home materializers to use the shared helper
  for memory projection result/event handling and text hashing.
- Added `test/test_provider_core_memory_projection.py`.
- Left OpenCode unchanged because its projection result/event model includes
  config merge fields and the `opencode_config_merge_failed` event.
- Restored Codex managed config `external_migration = false` overrides from
  `main` after agent3 flagged the regression.
- Added/updated tests covering the Codex feature override and helper edge cases.
- Sent the fixed Phase 1 patch back to agent3; agent3 approved it with no
  remaining concerns.
- Split `dev_tools/skills/ccb-github/scripts/check_release_state.py` into a
  stable CLI entrypoint plus `release_checker_shared.py`,
  `release_checker_markdown.py`, `release_checker_local.py`, and
  `release_checker_github.py`.
- Updated active `ccb-github` skill sync tracking to include the new helper
  modules.
- Added a regression test that asserts release checker helper modules are in
  `TRACKED_SKILL_FILES`.
- Sent Phase 2 to agent3; agent3 approved it with no blocking concerns.
- Applied agent3's non-blocking `semver_tuple` strict typing suggestion.
- Added decision 003 to keep reviewed `install.ps1` and `watch_fallback.py`
  surfaces active after reference searches.
- Extracted provider-home storage classification rules into
  `lib/storage_classification/provider_home.py`.
- Added direct provider-home classifier precedence/unknown-provider coverage.
- Ran post-phase `archi .` and recorded the comparison in
  `topics/post-phase-architec-results.md`.
- Sent the final architecture optimization pass to agent3; agent3 approved it
  with no blocking concerns.
- Ran the full test suite.
- Split `release_checker_github.py` further by moving workflow polling/status
  helpers to `release_checker_workflows.py` and release asset/published-state
  helpers to `release_checker_assets.py`.
- Updated `TRACKED_SKILL_FILES` and release checker tests for the new helper
  modules and monkeypatch targets.
- Split `lib/cli/management_runtime/startup_update.py` into a compatibility
  facade plus `startup_update_state.py`, `startup_update_refresh.py`, and
  `startup_update_flow.py`.
- Sent the provider-runtime follow-up plan to agent3. Agent3 approved
  prioritizing OpenCode projection before Claude binary cache and recommended
  a low-level projection event/marker primitive instead of widening the generic
  result schema.
- Added `write_projection_event_and_marker` and
  `materialize_provider_memory_file` to `lib/provider_core/memory_projection.py`.
- Reduced OpenCode projection recorder complexity while preserving the
  OpenCode-specific config-merge event.
- Moved shared Codex/Gemini memory bundle materialization into provider core.
- Split Claude binary cache routing into route-context and path-specific helper
  steps, keeping copy/link/version helpers unchanged.
- Ran final `archi .`: overall `54.75`, structure `67.14`, governance/full
  `42.37`. Remaining top hotspots are now close in score rather than dominated
  by the earlier release checker/startup/OpenCode items.

## Blockers

- None.

## Next Commit Target

Prepare the patch for commit.

## Last Verified Commands

- `archi .`
- `python -m compileall -q lib/provider_core/memory_projection.py lib/provider_profiles/codex_home_config.py lib/provider_backends/claude/launcher_runtime/home.py lib/provider_backends/gemini/launcher_runtime/home.py test/test_provider_core_memory_projection.py`
- `pytest test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py -x` (`175 passed in 1.58s`)
- `python -m compileall -q dev_tools/skills/ccb-github/scripts/check_release_state.py dev_tools/skills/ccb-github/scripts/release_checker_shared.py dev_tools/skills/ccb-github/scripts/release_checker_markdown.py dev_tools/skills/ccb-github/scripts/release_checker_local.py dev_tools/skills/ccb-github/scripts/release_checker_github.py test/test_ccb_github_skill.py`
- `pytest test/test_ccb_github_skill.py -x` (`12 passed in 0.16s`)
- `python dev_tools/skills/ccb-github/scripts/check_release_state.py --phase prepare --wait-seconds 0` (`OK: no blocking release-surface drift found.`, with expected dirty-worktree/upstream warnings)
- `pytest test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py test/test_ccb_github_skill.py -x` (`187 passed in 1.66s`)
- `git diff --check`
- `python -m compileall -q lib/storage_classification/service.py lib/storage_classification/provider_home.py test/test_storage_classification.py dev_tools/skills/ccb-github/scripts/release_checker_markdown.py`
- `pytest test/test_storage_classification.py -x` (`9 passed in 0.10s`)
- `pytest test/test_v2_cli_watch_reconnect.py test/test_windows_bootstrap_script.py -x` (`24 passed in 0.59s`)
- `pytest test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py test/test_ccb_github_skill.py test/test_storage_classification.py test/test_v2_cli_watch_reconnect.py test/test_windows_bootstrap_script.py -x` (`220 passed in 3.32s`)
- `archi .` (`overall=51.83`, `structure=65.77`, `governance/full=37.88`)
- `pytest -x` (`1903 passed in 157.67s`)
- `python -m compileall -q dev_tools/skills/ccb-github/scripts/check_release_state.py dev_tools/skills/ccb-github/scripts/release_checker_shared.py dev_tools/skills/ccb-github/scripts/release_checker_markdown.py dev_tools/skills/ccb-github/scripts/release_checker_local.py dev_tools/skills/ccb-github/scripts/release_checker_github.py dev_tools/skills/ccb-github/scripts/release_checker_workflows.py dev_tools/skills/ccb-github/scripts/release_checker_assets.py test/test_ccb_github_skill.py`
- `pytest test/test_ccb_github_skill.py -x` (`13 passed in 0.16s`)
- `python dev_tools/skills/ccb-github/scripts/check_release_state.py --phase prepare --wait-seconds 0` (`OK: no blocking release-surface drift found.`, with expected dirty-worktree/upstream warnings)
- `python dev_tools/skills/ccb-github/scripts/check_release_state.py --phase dev --wait-seconds 0` (exercised the dev workflow path; failed only on expected strict dirty-worktree/no-upstream/current GitHub workflow gates)
- `pytest test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py test/test_ccb_github_skill.py test/test_storage_classification.py test/test_v2_cli_watch_reconnect.py test/test_windows_bootstrap_script.py -x` (`221 passed in 2.12s`)
- `pytest test/test_cli_startup_update.py test/test_v2_cli_watch_reconnect.py test/test_windows_bootstrap_script.py -x` (`34 passed in 0.24s`)
- `pytest test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py -x` (`178 passed in 1.60s`)
- `pytest test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_v2_runtime_launch.py -x` (`163 passed in 1.34s`)
- `pytest test/test_cli_startup_update.py test/test_v2_cli_watch_reconnect.py test/test_windows_bootstrap_script.py test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py test/test_ccb_github_skill.py test/test_storage_classification.py -x` (`234 passed in 1.95s`)
- `git diff --check`
- `archi --diff .` (`overall=56.72`, `structure=67.14`, `governance=46.29`)
- `archi .` (`overall=54.75`, `structure=67.14`, `governance/full=42.37`)

## Handoff Notes

Treat the generated `.architec/` artifacts as analysis input, not source
changes. The worktree currently also contains an untracked `.ccb-workspace.json`
that is unrelated to this planning tree.
