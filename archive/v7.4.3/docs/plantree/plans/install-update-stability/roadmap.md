# Install Update Stability Roadmap

Date: 2026-06-04

## Done

- Existing installer detects `CCB_LANG`/locale and has a `msg` function for
  selected shell installer messages.
- Existing installer blocks accidental root installs unless the user confirms
  interactively or sets `CCB_ALLOW_ROOT_INSTALL=1`.
- Existing installer selects Python 3.10+, creates managed venvs for release
  installs where requested, and writes Python wrappers for Python entrypoints.
- Existing installer runs installed entrypoint smoke checks:
  `$BIN_DIR/ccb --print-version` and `$BIN_DIR/ask --help`.
- Existing install/update flows make Neovim and Role Pack provisioning optional
  by default and skip them in non-interactive mode.
- Existing Role Pack catalog status can detect `current`, `available`,
  `update_available`, and `installed_source_missing`.
- Existing Role Pack implementation canonicalizes `ccb.archi` input to
  `agentroles.archi` in most runtime and CLI paths.
- Captured the v7.2.9 post-update `ccb.archi` failure in
  [history/v729-rolepack-update-failure-2026-06-04.md](history/v729-rolepack-update-failure-2026-06-04.md).
- `ccb update` now delegates post-update Role Pack and Neovim provisioning to
  the newly installed `ccb __post-update` entrypoint after tarball install and
  entrypoint verification; subprocess provisioning failure is reported as a
  warning without failing the core update.
- Post-update delegation now prefers the installed bin wrapper or explicit
  `CODEX_BIN_DIR` so managed Python environments stay in effect; forced
  provisioning failures can still fail the update.
- Added Role Pack legacy store canonicalization for installed `ccb.archi`
  metadata, including safe canonical metadata repair under `agentroles.archi`
  and fallback to the catalog source when old `source_path` values are gone.
- Added regression coverage that catalog `current` Role Packs do not call
  update hooks, and that inherited `ccb-config` docs use `ccb.archi` only as a
  legacy alias.

## In Progress

- Define no-repeat provisioning contracts for dependencies and Role Packs.
- Define Chinese/English prompt coverage for shell installer and Python update
  paths.

## Next

1. Complete broader Role Pack provisioning idempotency by status:
   `update_available` updates exactly once and missing catalog is a warning
   unless required.
2. Make role-owned tool hooks idempotent by tool manifest/version so unchanged
   Architec or future tools are not repeatedly installed.
3. Consolidate update/install prompts behind shared i18n message helpers for
   Chinese and English.
4. Add automated tests for every scenario in
   [topics/environment-and-scenario-matrix.md](topics/environment-and-scenario-matrix.md).
5. Add a real upgrade runbook that starts from an older released version with
   installed legacy `ccb.archi`, updates to the new release, and confirms no
   user-facing Role Pack error.

## Deferred

- Windows-native managed update.
- Signed installer/update manifests.
- Global background dependency update checks.
- Full provider CLI installation management.
- Automatic cleanup of obsolete installed Role Pack digest versions.
