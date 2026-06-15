# Installed CCB Owns The Work Environment

Date: 2026-06-09

## Context

The source checkout `/home/bfly/yunwei/ccb_source` is also used as a live CCB
collaboration project. When source edits, source runtime tests, and installed
work-environment commands share wrappers or runtime state, a code change can
appear to affect the current development environment or other CCB projects.

The repo already has guarded source entrypoints, but operational instructions
and runbooks still need a durable authority statement.

## Decision

The installed-release `ccb` is the authority for normal work-environment CCB
collaboration in `ccb_source`. Current source changes are validated only
through `/home/bfly/yunwei/ccb_source/ccb_test` from the default external test
project `/home/bfly/yunwei/test_ccb2`, unless another external root is
explicitly allowed with `CCB_TEST_ROOTS` or `CCB_SOURCE_ALLOWED_ROOTS`.

Source validation must not delete or rewrite `ccb_source/.ccb/agents`,
`ccb_source/.ccb/ccbd`, provider-state directories, or global/system wrappers.
Promotion from source to the installed environment is an explicit install,
update, or release action after validation gates pass.

## Consequences

- Agents should not suggest bare `ccb` or source `./ccb` commands for
  source-change validation.
- Runbooks should use the absolute source `ccb_test` wrapper or first prove the
  bare command resolves to that wrapper.
- Cleanup of project agents in `ccb_source` is work-environment maintenance,
  not source-test cleanup.
- Future implementation can narrow default allowed test roots or add wrapper
  diagnostics without changing the operator contract.
