# Storage Classification And Cleanup Plan

Date: 2026-05-18

## Purpose

Improve cleanup governance without deleting or archiving live runtime,
documentation, or installer surfaces based on heuristic labels alone.

## Current Inventory

Architec cleanup summary:

- Candidates: `108`
- Review required: `108`
- Categories:
  - `fallback_branch=80`
  - `stale_doc=26`
  - `obsolete_script=1`
  - `compat_layer=1`

Semantic judge summary:

- `keep_active=8`
- `review=2`
- Most reviewed documentation is current or already archived historical
  reference.
- `install.ps1` and `lib/cli/services/watch_fallback.py` require manual review.

Primary storage classifier:

- `lib/storage_classification/service.py`

Existing tests:

- `test/test_storage_classification.py`

## Interpretation

The cleanup score is low, but the cleanup inventory is not safe as a work queue.
The classifier and semantic judge disagree on several high-profile files:

- `README.md` is a live project entrypoint.
- `CHANGELOG.md` has an active unreleased/recent release structure.
- multiple `docs/*contract*.md` files are authoritative runtime contracts.
- files already under `archive/` should not be moved to `archive/archive/`.

The cleanup effort should therefore improve decision quality first.

## Target Shape

Storage classification:

- keep deterministic precedence from the provider-state storage boundary plan;
- reduce imperative complexity by grouping provider path rules into small
  provider-local classifiers or table-driven predicates;
- keep test-visible output stable.

Cleanup governance:

- classify candidates into `keep_active`, `review`, `archive`, and `delete`;
- require caller/import/reference evidence before archiving operational files;
- never archive authority docs or active fallback source on heuristic evidence
  alone.

## Reviewed Cleanup Ledger

The Phase 3 reference search resolved the two semantic-judge review items:

| Path | Heuristic | Decision | Evidence |
| --- | --- | --- | --- |
| `install.ps1` | `obsolete_script` | `keep_active` | Referenced by `README.md`, `README_zh.md`, `install.sh`, `install.cmd`, `scripts/bootstrap-windows-test-env.ps1`, `lib/cli/management_runtime/install.py`, `docs/ccb-release-packaging-plan.md`, and `test/test_windows_bootstrap_script.py`. |
| `lib/cli/services/watch_fallback.py` | `fallback_branch` | `keep_active` | Imported by `lib/cli/services/watch_runtime.py` and `lib/cli/services/ask_runtime/watch.py`; fallback behavior is covered by `test/test_v2_cli_watch_reconnect.py`. |

See
[../decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md](../decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md).

No archive or delete actions are part of Phase 3.

## Sequence

1. Leave all cleanup candidates in place.
2. Add a reviewed cleanup ledger or topic section for `install.ps1` and
   `watch_fallback.py` after reference searches. Done in Phase 3.
3. Refactor `storage_classification/service.py` by extracting provider-specific
   classifiers while preserving `summarize_storage` output.
4. Add tests for any newly explicit cleanup rule.
5. Re-run `archi .` and compare cleanup hygiene and hotspot movement.

## Verification

```bash
pytest test/test_storage_classification.py
rg -n "install.ps1|watch_fallback" .
archi .
```

## Do Not Archive Without New Evidence

- `README.md`
- `README_zh.md`
- `CHANGELOG.md`
- `docs/ccbd-startup-supervision-contract.md`
- `docs/ccb-config-layout-contract.md`
- `docs/ccb-provider-state-storage-boundary-plan.md`
- provider session isolation contracts
- `docs/ccbd-manual-test-issue-log.md`
- files already under `archive/`
