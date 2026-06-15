# Repository Cleanup And Filesystem Plan

Date: 2026-06-09

## Purpose

Define what can and cannot be cleaned while separating source development from
the installed CCB work environment.

## Current Inventory

- `/home/bfly/yunwei/ccb_source` is the source checkout and also has an active
  installed-release CCB work environment under `.ccb`.
- On 2026-06-09, `.ccb/ccbd/lifecycle.json` in `ccb_source` reported
  `desired_state: running`, `phase: mounted`, and generation `161`.
- On 2026-06-09, `.ccb` in `ccb_source` was about `1.5G`, with active agent
  directories for `main`, `bugb`, `worker1`, `worker2`, `worker3`, `reviewer1`,
  `reviewer2`, `reviewer3`, `archi`, `push`, and `coworker`.
- `/home/bfly/yunwei/test_ccb2` is the only default external
  source-validation project and already contains `.ccb`, provider homes, and
  `source_home`.
- Other sibling test directories exist under `/home/bfly/yunwei`, including
  `test_ccb`, `ccb_test2`, `test_ccb_provider_memory_matrix`, and timestamped
  smoke directories. They are not default source-test roots; use
  `CCB_TEST_ROOTS` or `CCB_SOURCE_ALLOWED_ROOTS` when one is intentionally
  under test.
- On 2026-06-09, `PATH` resolved bare `ccb` and `ccb_test` to
  `/tmp/ccb-v7.2.1-install-smoke/prefix` before `~/.local/bin`. Treat bare
  wrapper commands as ambiguous until verified.

## Target Structure

- `ccb_source`: source files, durable docs/plans, and installed-release
  work-environment `.ccb` state only.
- `test_ccb2`: default stateful source-under-test project.
- Temporary install/update simulations: isolated homes and prefixes outside
  `ccb_source`, preferably disposable directories under `/tmp` or clearly named
  sibling test directories.

## Keep / Move / Archive / Delete Rules

- Keep `.ccb/ccb.config`, `.ccb/ccb_memory.md`, `AGENTS.md`, and active
  provider-state in `ccb_source` unless the task is explicitly to reset the
  work environment.
- Do not delete `ccb_source/.ccb/agents/*` or `ccb_source/.ccb/ccbd/*` during
  source validation.
- Test-project runtime residue may be cleaned only after the corresponding
  test backend is stopped.
- Historical test directories should be archived or deleted only after their
  purpose, owner, and rollback value are recorded.
- Global wrappers and shell PATH should be audited before repair; do not
  silently repoint system `ccb` to a source checkout.
- Do not add `CCB_SOURCE_ALLOWED_ROOTS` or `CCB_TEST_ROOTS` to persistent shell
  startup files. Use them only around one explicit source-validation command or
  script.

## Cleanup Sequence

1. Record `git status --short` in `ccb_source`.
2. Record wrapper resolution with `command -v ccb`, `command -v ccb_test`, and
   `readlink -f`.
3. If cleaning `/home/bfly/yunwei/test_ccb2`, first run:

   ```bash
   cd /home/bfly/yunwei/test_ccb2
   /home/bfly/yunwei/ccb_source/ccb_test kill
   ```

4. Remove only test-project runtime artifacts that are known generated state,
   such as its `.ccb/ccbd`, `.ccb/agents`, and provider session files.
5. Recreate or validate the test anchor with:

   ```bash
   cd /home/bfly/yunwei/test_ccb2
   HOME=/home/bfly/yunwei/test_ccb2/source_home \
   CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home \
   /home/bfly/yunwei/ccb_source/ccb_test config validate
   ```

## Safety Checks

- `./ccb doctor` from `ccb_source` should refuse stateful source-checkout
  execution unless an explicit diagnostic override is set.
- `/home/bfly/yunwei/ccb_source/ccb_test doctor` should refuse when run from
  `ccb_source`.
- The same `ccb_test doctor` should run from `/home/bfly/yunwei/test_ccb2`.
- `/home/bfly/yunwei/ccb_source/ccb_test --diagnose` should show the source
  wrapper, effective roots, and `allowed_source_test_project: yes` from
  `/home/bfly/yunwei/test_ccb2`.
- Installed work-environment `ccb` should not import from
  `/home/bfly/yunwei/ccb_source/lib`.
- No cleanup task should leave another CCB project pointing at
  `/home/bfly/yunwei/ccb_source` as its runtime implementation.
