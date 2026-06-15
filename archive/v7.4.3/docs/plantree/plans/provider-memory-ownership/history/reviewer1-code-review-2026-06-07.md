# Reviewer1 Code Review

Date: 2026-06-07

Reviewer: reviewer1

## Result

Reviewer1 found no correctness blockers in the first implementation.

## Follow-Ups Applied

- Replaced a provider-user-memory magic string in
  `lib/provider_core/memory_projection.py` with the policy constant.
- Removed Claude's remaining explicit provider-native project exclusion flag so
  provider policy is the single source of truth.
- Refined provider-user-memory filtering so user-authored paragraph spacing is
  preserved after CCB install blocks are removed.
- Added tests for all recognized install marker pairs, Chinese legacy
  collaboration-rule sections, realistic temporary project composition, and
  opt-in external real project context inspection.
- Added seed-aware shared-memory upgrade for unedited generated old templates
  after the opt-in `/home/bfly/yunwei/test_ccb2` inspection exposed stale v4
  shared memory embedded in existing provider-state.

## Remaining Open Work

- External runtime validation still needs regenerated managed provider memory
  files from a source-under-test `ccb_test` run in an external project.
