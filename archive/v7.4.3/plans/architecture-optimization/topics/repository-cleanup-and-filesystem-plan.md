# Repository Cleanup And Filesystem Plan

Date: 2026-05-18

## Purpose

Define file movement, generated artifact, and cleanup safety rules for the
architecture optimization work.

## Current Inventory

Planning tree added:

```text
plans/architecture-optimization/
  README.md
  roadmap.md
  implementation-status.md
  open-questions.md
  topics/
  decisions/
```

Generated analysis artifacts:

```text
.architec/
```

Observed unrelated untracked file:

```text
.ccb-workspace.json
```

Architec topology review:

- `lib/provider_model_shortcuts.py`: review-only root placement
- `lib/release_artifacts.py`: review-only root placement

## Target Structure

No source-tree package moves are part of the first optimization phase.

Potential future namespaces need a separate decision before implementation:

- `lib/provider/` or a more specific provider metadata namespace for
  `provider_model_shortcuts.py`
- `lib/release/` for `release_artifacts.py`

## Keep / Move / Archive / Delete Rules

Keep active:

- authoritative runtime contracts in `docs/`;
- top-level README and changelog files;
- runtime fallback source until caller evidence proves it is dead;
- installer scripts until support status is decided;
- `install.ps1`, because Phase 3 reference search confirmed it is still a
  Windows install surface;
- `lib/cli/services/watch_fallback.py`, because Phase 3 reference search
  confirmed it is still called by watch paths and covered by reconnect tests.

See
[../decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md](../decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md).

Move only after explicit decision:

- root-level implementation modules flagged by topology review;
- release checker helper modules out of the current script.

Archive only after:

- reference search;
- import/caller search;
- documentation link check;
- rollback path;
- owner decision recorded in this tree or a contract doc.

Delete only when:

- the file is generated/disposable; or
- an archive period has elapsed and deletion is explicitly requested.

## Generated And Runtime Files

`.architec/` is analysis output. It can be regenerated with `archi .` and
should not be treated as an authoritative source contract.

`.ccb-workspace.json` was present before this plan was written. Do not remove
or modify it as part of architecture optimization.

## Legacy Freeze Rules

Do not clean up files merely because their text contains "legacy", "obsolete",
"migration", or "fallback". In this repository those terms often describe
current non-drift behavior or recovery semantics.

## Cleanup Sequence

1. Review `install.ps1` support status. Done in Phase 3.
2. Review `watch_fallback.py` caller path. Done in Phase 3.
3. Refactor storage classification so cleanup summaries can point to stronger
   evidence.
4. Only then consider archive moves.

## Safety Checks

Before moving or archiving any file:

```bash
git status --short
rg -n "<path-or-symbol>" .
pytest <focused-test-slice>
```

After structural source moves:

```bash
pytest
archi .
```
