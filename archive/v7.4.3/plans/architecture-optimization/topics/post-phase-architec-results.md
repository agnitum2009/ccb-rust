# Post-Phase Architec Results

Date: 2026-05-18

## Snapshot

After Phases 1-3, `archi .` reported:

- Overall: `51.83` (baseline: `50.11`)
- Governance overall: `37.88` (baseline: `35.39`)
- Structure: `65.77` (baseline: `64.83`)
- Full: `37.88` (baseline: `35.39`)

The score moved in the intended direction, but Architec still reports mid-range
governance because complexity remains concentrated in provider backends, CLI
runtime surfaces, and the newly split release checker helper modules.

## Hotspot Movement

Baseline top hotspots:

1. `dev_tools/skills/ccb-github/scripts/check_release_state.py`
2. `lib/provider_profiles/codex_home_config.py`
3. `lib/storage_classification/service.py`
4. `lib/provider_backends/gemini/launcher_runtime/home.py`
5. `lib/provider_backends/claude/launcher_runtime/home.py`

Post-phase top hotspots:

1. `dev_tools/skills/ccb-github/scripts/release_checker_github.py`
2. `lib/provider_backends/opencode/launcher.py`
3. `lib/provider_backends/claude/launcher_runtime/binary_cache.py`
4. `lib/cli/management_runtime/startup_update.py`
5. `lib/provider_profiles/codex_home_config.py`

The release checker entrypoint is no longer the top hotspot, but
`release_checker_github.py` is now rank 1. That is acceptable for this pass
because Phase 2 intentionally kept GitHub release and workflow behavior together
after splitting the stable CLI entrypoint and skill projection surface.

`lib/storage_classification/service.py` moved from rank 3 to rank 8 after
provider-home rules moved into `lib/storage_classification/provider_home.py`.

## Cleanup Interpretation

The post-phase cleanup inventory still reports all cleanup candidates as review
required:

- Candidates: `112`
- Review required: `112`
- Categories: `compat_layer=1`, `fallback_branch=80`, `obsolete_script=1`,
  `stale_doc=30`

The semantic judge suggested `archive_first` only for planning notes under the
active architecture optimization tree. Those files should remain in place until
the optimization work is committed or explicitly archived. Active project
surfaces such as `README.md`, `CHANGELOG.md`, authoritative contracts,
`install.ps1`, and `watch_fallback.py` must not be archived from heuristic
labels alone.

## Follow-Up Optimization

The first follow-up target has been implemented: `release_checker_github.py`
was reduced from `669` lines to `402` lines by moving workflow handling into
`release_checker_workflows.py` and release asset/published-state handling into
`release_checker_assets.py`.

The remaining highest-complexity function in `release_checker_github.py` is
`check_github`, which intentionally remains the top-level GitHub release
orchestrator for compatibility and behavior clarity.

## Next Optimization Targets

- Split `lib/cli/management_runtime/startup_update.py` into cache-state,
  background-refresh, and prompt/relaunch helpers.
- Split Claude binary cache routing into scan, plan, and execute steps.
- Evaluate `lib/provider_backends/opencode/launcher.py` with the OpenCode
  completion contract before extracting helpers.
