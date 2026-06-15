# Architec Baseline Diagnosis

Date: 2026-05-18

## Baseline

The full Architec run reports:

- Overall: `50.11`
- Governance overall: `35.39`
- Structure: `64.83`
- Full: `35.39`

Structure is materially healthier than governance. The repository does not need
a broad package reshuffle first: boundary clarity, coupling control, and package
topology all scored `100.0`. The main architectural drag is concentrated in
complexity hotspots, wide components, and unsafe cleanup signals.

## Hotspots

Top hotspots:

1. `dev_tools/skills/ccb-github/scripts/check_release_state.py`
2. `lib/provider_profiles/codex_home_config.py`
3. `lib/storage_classification/service.py`
4. `lib/provider_backends/gemini/launcher_runtime/home.py`
5. `lib/provider_backends/claude/launcher_runtime/home.py`
6. `lib/provider_backends/opencode/launcher.py`
7. `lib/provider_backends/claude/launcher_runtime/binary_cache.py`
8. `lib/cli/management_runtime/startup_update.py`

The dominant metric is cyclomatic complexity. The files are not just long; they
mix multiple policy and IO responsibilities in one runtime surface.

## Risk Components

Highest-risk components:

- `lib:provider_backends`: risk `341.35`, 285 files
- `lib:ccbd`: risk `253.85`, 164 files
- `lib:cli`: risk `196.6`, 106 files
- `lib:agents`: risk `46.9`, 27 files
- `lib:provider_profiles`: risk `39.6`, 4 files

`provider_backends` has the highest risk because each provider implements
overlapping concepts separately: managed home resolution, config projection,
auth inheritance, memory projection, cache routing, session layout, and event
recording.

## Code Findings

Provider materialization:

- Codex, Claude, and Gemini each define similar `_memory_projection_result`,
  `_record_memory_projection_event`, `_same_memory_projection_signature`, and
  `_text_file_sha256` helpers.
- Provider-specific code correctly owns auth/config semantics. That should not
  be moved into a generic abstraction until the shared contract is explicit.
- Existing tests in `test/test_provider_profiles.py` and
  `test/test_provider_hook_settings.py` already protect many projection cases.

Storage classification:

- `lib/storage_classification/service.py` follows the storage boundary plan but
  encodes provider rules as nested imperative conditionals.
- Tests in `test/test_storage_classification.py` cover key class separation:
  session, secret, projected config, startup authority bundle, rebuildable
  cache, user content, workspace, and runtime ephemeral.

Release checker:

- `check_release_state.py` combines local git state, local release file
  validation, README surface checks, GitHub API calls, workflow polling, release
  asset verification, and CLI output.
- Existing tests in `test/test_ccb_github_skill.py` cover a subset of behavior
  and should be preserved during extraction.

## Cleanup Interpretation

Architec identified 108 cleanup candidates, but all require review. The
semantic judge kept most reviewed documentation active and explicitly called out
`install.ps1` and `lib/cli/services/watch_fallback.py` for manual review.

Do not treat archive candidates as an action list. Many active contract
documents contain words like "legacy", "obsolete", or "migration" because they
document current non-drift rules and historical boundaries.

## Optimization Reading

The first optimization should improve governance without disturbing strong
structure signals:

- extract repeated provider materialization mechanisms;
- keep provider-specific behavior local;
- split the release checker by responsibility;
- convert cleanup heuristics into reviewed rules;
- re-run Architec after each phase to confirm hotspot movement.
