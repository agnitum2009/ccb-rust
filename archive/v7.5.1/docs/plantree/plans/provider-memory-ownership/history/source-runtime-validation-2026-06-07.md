# Source Runtime Validation

Date: 2026-06-07

Project: `/home/bfly/yunwei/test_ccb2`

Source entrypoint: `/home/bfly/yunwei/ccb_source/ccb_test`

## Initial Finding

The PATH `ccb_test` in `/home/bfly/yunwei/test_ccb2` resolved to a release
install under `/tmp/ccb-v7.2.1-install-smoke/prefix`, so the final validation
used the source checkout wrapper by absolute path.

The first source runtime start attempt exposed an unrelated keeper startup
blocker:

```text
ImportError: cannot import name 'set_tmux_ui_active' from partially initialized module 'cli.services.tmux_ui'
```

The cycle was:

```text
ccbd keeper -> project namespace materialization -> cli.services.tmux_ui
-> tmux_ui_runtime.helpers -> cli.management_runtime
-> management update command -> cli.services.tmux_ui
```

## Fix Applied

`tmux_ui_runtime.helpers` now reads local version metadata directly from
`BUILD_INFO.json`, `VERSION`, or embedded `ccb` assignments, instead of
importing `cli.management_runtime`.

Regression coverage:

```bash
pytest -q test/test_v2_tmux_ui.py
```

## Shared Memory Migration Finding

`/home/bfly/yunwei/test_ccb2/.ccb/ccb_memory.md` still exactly matched the old
generated v4 template, but `memory.seed.json` had been removed by runtime
cleanup. Metadata-only upgrade was therefore insufficient.

Fix applied:

- Unedited generated old templates still upgrade when `memory.seed.json` proves
  the file is unchanged.
- Exact known generated legacy templates also upgrade when seed metadata is
  missing.
- User-edited files remain untouched.

## Validation Commands

```bash
cd /home/bfly/yunwei/test_ccb2
/home/bfly/yunwei/ccb_source/ccb_test doctor
/home/bfly/yunwei/ccb_source/ccb_test
```

Result:

- `doctor` reported `install_mode: source`.
- Source start succeeded with `start_status: ok`.
- `ccbd_started: true`.
- Agents: `agent1, archi`.

```bash
CCB_REAL_PROJECT_MEMORY_CHECK=1 CCB_REAL_TEST_PROJECT=/home/bfly/yunwei/test_ccb2 pytest -q test/test_provider_memory_external_context.py
```

Result: 1 passed.

Generated evidence after source runtime regeneration:

- `.ccb/ccb_memory.md` upgraded to the v5 short template.
- Codex generated `AGENTS.md` for `agent1` and `archi` each contains exactly one
  `CCB Runtime Coordination Rules` section and no `Ask Communication` section.
- Codex generated bundles include `.ccb/ccb_memory.md` and exclude project
  `AGENTS.md`.

## Final Automated Validation

```bash
pytest -q test/test_project_memory.py test/test_project_memory_filters.py test/test_project_memory_real_context.py test/test_provider_memory_external_context.py test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_v2_runtime_launch.py test/test_v2_tmux_ui.py
```

Result: 236 passed, 1 skipped.
