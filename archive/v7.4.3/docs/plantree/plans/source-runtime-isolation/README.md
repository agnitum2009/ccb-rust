# Source Runtime Isolation Plan

Date: 2026-06-09

## Purpose

Keep CCB source editing, source-under-test validation, and stable installed
work-environment usage separated. The target developer flow is:

1. Edit source in `/home/bfly/yunwei/ccb_source`.
2. Validate current source through `/home/bfly/yunwei/ccb_source/ccb_test`.
3. Run stateful source tests only from `/home/bfly/yunwei/test_ccb2`.
4. Keep normal project collaboration on the installed-release `ccb`.

This prevents a live source checkout or source test from changing the current
development CCB environment or other CCB projects.

## File Map

- [roadmap.md](roadmap.md): current readiness and follow-up implementation
  sequence.
- [topics/development-workflow.md](topics/development-workflow.md): operator
  workflow, command lanes, and validation contract.
- [topics/repository-cleanup-and-filesystem-plan.md](topics/repository-cleanup-and-filesystem-plan.md):
  cleanup rules for source checkout runtime state, test projects, and wrapper
  path hygiene.
- [decisions/001-installed-ccb-authority.md](decisions/001-installed-ccb-authority.md):
  stable installed `ccb` remains the work-environment authority.

## Related Sources

- [../../../ccb](../../../ccb)
- [../../../ccb_test](../../../ccb_test)
- [../../../test/test_source_runtime_guard.py](../../../test/test_source_runtime_guard.py)
- [../../baseline/test-and-release-gates.md](../../baseline/test-and-release-gates.md)
- [../install-update-stability/topics/validation-runbook.md](../install-update-stability/topics/validation-runbook.md)

## Scope

In scope:

- Source checkout entrypoint discipline.
- `ccb_test` source-under-test workflow and test-project boundaries.
- `/home/bfly/yunwei/test_ccb2` as the default dedicated stateful test anchor.
- Project memory and runbook wording that prevents agents from using source
  runtime commands in the work environment.
- Cleanup policy for source checkout `.ccb` runtime state and test-project
  residue.

Out of scope:

- Publishing a release or changing the globally installed `ccb`.
- Deleting active project agents in `/home/bfly/yunwei/ccb_source/.ccb`.
- Replacing provider authentication or provider-native account configuration.
- Migrating every historical plan note that mentions older `ccb_test` command
  examples.

## Non-Drift Contract

- `/home/bfly/yunwei/ccb_source` is source code plus an installed-release work
  environment, not the stateful source-test project.
- Source changes must not change what the installed-release `ccb` imports for
  normal collaboration.
- Stateful source validation uses `/home/bfly/yunwei/ccb_source/ccb_test` from
  `/home/bfly/yunwei/test_ccb2` by default. Any other external test project
  requires an explicit `CCB_TEST_ROOTS` or `CCB_SOURCE_ALLOWED_ROOTS`
  override.
- Runbooks should prefer the absolute source `ccb_test` wrapper because `PATH`
  can contain old release or smoke-test wrappers.
- `ccb_test --diagnose` is the lightweight preflight for wrapper/root
  ambiguity; it must not start or mutate a project backend.
- Provider/account state for source validation should use
  `/home/bfly/yunwei/test_ccb2/source_home` through `HOME` and
  `CCB_SOURCE_HOME`, unless the test intentionally exercises inherited real
  provider configuration.
- `.ccb/agents/*`, `.ccb/ccbd/*`, tmux sockets, and provider-state directories
  under the source checkout are work-environment runtime state. They are not
  source-test cleanup targets.
- `CCB_SOURCE_RUNTIME_OK=1` is a narrow diagnostics override, not a normal
  developer workflow; agents should not set it unless the user explicitly asks
  for that diagnostic bypass.
