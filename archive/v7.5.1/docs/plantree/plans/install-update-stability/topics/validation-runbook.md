# Validation Runbook

Date: 2026-06-04

## Automated Tests

Targeted suites:

```bash
pytest -q \
  test/test_install_identity_output.py \
  test/test_install_root_confirmation.py \
  test/test_install_script_sidebar.py \
  test/test_cli_management_update.py \
  test/test_rolepacks.py \
  test/test_source_runtime_guard.py \
  test/test_build_linux_release_script.py
```

Covered by targeted tests:

- update post-provisioning delegates to newly installed `ccb`
- old updater does not run Role Pack update semantics after install
- post-update delegation prefers the installed bin wrapper or explicit
  `CODEX_BIN_DIR` before falling back to the raw install entrypoint
- post-update optional provisioning failure stays a warning, while forced
  provisioning failure returns failure
- forced Role Pack provisioning returns failure when catalog refresh fails,
  installed role update fails, or a selected new role install fails
- `CCB_POST_UPDATE_REQUIRED=1` auto-accepts Role Pack provisioning and marks
  Neovim provisioning required without prompting in TTY sessions
- legacy installed `ccb.archi` migrates to `agentroles.archi`
- legacy installed `ccb.archi` also migrates on direct role status queries
- stale `source_path` falls back to catalog
- Role Pack `current` skips update hooks
- inherited `ccb-config` Codex and Claude skill docs use canonical
  `agentroles.archi` examples and mention `ccb.archi` only as a legacy input
  alias

Remaining tests to add:

- tool manifest current skips repeated install
- `CCB_LANG=zh` and `CCB_LANG=en` for install/update prompts
- non-interactive update skips optional provisioning and prints retry commands

## Local Release Simulation

Use isolated homes:

```bash
export HOME=/tmp/ccb-install-home
export XDG_DATA_HOME=/tmp/ccb-install-home/.local/share
export XDG_CACHE_HOME=/tmp/ccb-install-home/.cache
export CODEX_INSTALL_PREFIX=/tmp/ccb-install-home/.local/share/codex-dual
export CODEX_BIN_DIR=/tmp/ccb-install-home/.local/bin
```

Scenarios:

1. Fresh release install with default optional prompts accepted.
2. Fresh release install with `CCB_INSTALL_ROLES=0` and `CCB_INSTALL_NEOVIM=0`.
3. Non-interactive fresh install.
4. Update from an older release with no Role Packs installed.
5. Update from an older release with canonical `agentroles.archi` installed.
6. Update from an older release with legacy `ccb.archi` installed.
7. Update with catalog unavailable but cache already present.
8. Update with catalog unavailable and no cache.
9. Root install non-interactive failure.
10. Root install explicit override.

## Real Project Smoke

Use the dedicated disposable project outside `ccb_source` at
`/home/bfly/yunwei/test_ccb2`. When validating current source changes from this
checkout, use the absolute source wrapper
`/home/bfly/yunwei/ccb_source/ccb_test`, not the installed `ccb`. Do not rely
on a bare `ccb_test` until `command -v ccb_test` and `readlink -f` prove it is
the source wrapper.

Commands:

```bash
cd /home/bfly/yunwei/test_ccb2
export HOME=/home/bfly/yunwei/test_ccb2/source_home
export CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home
/home/bfly/yunwei/ccb_source/ccb_test --diagnose
/home/bfly/yunwei/ccb_source/ccb_test doctor
/home/bfly/yunwei/ccb_source/ccb_test roles list
/home/bfly/yunwei/ccb_source/ccb_test roles install agentroles.archi
/home/bfly/yunwei/ccb_source/ccb_test roles doctor agentroles.archi
/home/bfly/yunwei/ccb_source/ccb_test roles add agentroles.archi:codex --window main
/home/bfly/yunwei/ccb_source/ccb_test
/home/bfly/yunwei/ccb_source/ccb_test reload
/home/bfly/yunwei/ccb_source/ccb_test doctor
/home/bfly/yunwei/ccb_source/ccb_test kill
```

Expected:

- no `ccb.archi` user-facing failure
- no repeated role/tool installation when role/tool is current
- `archi` appears as project agent name
- role id remains `agentroles.archi`
- runtime can mount and cleanly stop

## Release Gate

Before publishing:

- targeted suites pass
- `compileall` or equivalent syntax check passes
- `git diff --check` passes
- Linux release build passes
- update from previous stable release passes in an isolated home
- update from a legacy Role Pack state passes in an isolated home
- Chinese and English prompt checks pass
- `inherit_skills/{codex_skills,claude_skills}/ccb-config/` is synchronized
  with any config/usage changes introduced by the release
