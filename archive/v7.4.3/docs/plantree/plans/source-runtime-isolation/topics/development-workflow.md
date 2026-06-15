# Development Workflow

Date: 2026-06-09

## Goal

Make every CCB development lane explicit:

- edit source in `/home/bfly/yunwei/ccb_source`
- validate source with `/home/bfly/yunwei/ccb_source/ccb_test`
- run stateful validation from `/home/bfly/yunwei/test_ccb2`
- keep normal collaboration on the installed-release `ccb`

## Work-Environment Lane

Use this lane for normal project collaboration in `ccb_source`.

- The active `.ccb` backend, configured agents, provider sessions, and tmux
  state belong to the installed-release work environment.
- Use installed-release `ccb` and `ask` for ordinary collaboration.
- Do not run `./ccb`, `python ccb`, or source `ccb_test` from this checkout for
  normal work-environment commands.
- Do not delete `.ccb/agents/*` or `.ccb/ccbd/*` as source cleanup. If the
  work environment itself must be reset, do that as an explicit installed-ccb
  maintenance task.

## Source-Under-Test Lane

Use this lane after modifying source.

```bash
cd /home/bfly/yunwei/test_ccb2
export HOME=/home/bfly/yunwei/test_ccb2/source_home
export CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home
/home/bfly/yunwei/ccb_source/ccb_test doctor
/home/bfly/yunwei/ccb_source/ccb_test config validate
```

For stateful smoke tests, keep using the same absolute source wrapper:

```bash
cd /home/bfly/yunwei/test_ccb2
export HOME=/home/bfly/yunwei/test_ccb2/source_home
export CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home
/home/bfly/yunwei/ccb_source/ccb_test
/home/bfly/yunwei/ccb_source/ccb_test doctor
/home/bfly/yunwei/ccb_source/ccb_test kill
```

The absolute wrapper matters because `PATH` can contain a release or smoke-test
copy of `ccb_test`. If a runbook intentionally uses a bare command, first
record:

```bash
command -v ccb_test
readlink -f "$(command -v ccb_test)"
```

Use the wrapper diagnostic when the path or root selection is uncertain:

```bash
cd /home/bfly/yunwei/test_ccb2
/home/bfly/yunwei/ccb_source/ccb_test --diagnose
```

`/home/bfly/yunwei/test_ccb2` is the only default stateful source-test root.
Other external projects must be explicitly allowed:

```bash
export CCB_TEST_ROOTS=/path/to/temporary-source-test-project
/home/bfly/yunwei/ccb_source/ccb_test config validate
```

## Update And Promotion Lane

Source validation does not update the installed work environment. Promotion is
a separate operation:

- Release simulations use isolated `HOME`, `XDG_*`, `CODEX_INSTALL_PREFIX`,
  and `CODEX_BIN_DIR` values.
- Managed `ccb update` validation belongs in isolated install/update smoke
  projects, not in the source checkout's live `.ccb` state.
- The global/system `ccb` should be changed only after source validation,
  release packaging, and install/update gates are accepted.
- Do not leave `CCB_SOURCE_ALLOWED_ROOTS` or `CCB_TEST_ROOTS` in a long-lived
  shell profile. They are per-test overrides for source validation.

## Guardrails

- `ccb_test` must not run from `/home/bfly/yunwei/ccb_source`.
- `ccb_test --project` must not point inside `/home/bfly/yunwei/ccb_source`.
- `ccb_test` must not run from sibling legacy test directories such as
  `/home/bfly/yunwei/test_ccb` or `/home/bfly/yunwei/ccb_test2` unless
  `CCB_TEST_ROOTS` or `CCB_SOURCE_ALLOWED_ROOTS` explicitly allows them.
- `CCB_SOURCE_RUNTIME_OK=1` is only for explicit diagnostics; do not set it for
  ordinary development validation.
- `HOME` and `CCB_SOURCE_HOME` should point at the test project's
  `source_home` unless the test is specifically about inherited real provider
  configuration.
- Before a long stateful test, verify no stale test backend is still running:

```bash
cd /home/bfly/yunwei/test_ccb2
/home/bfly/yunwei/ccb_source/ccb_test doctor
```
