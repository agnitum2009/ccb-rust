# Final Review Fix

Date: 2026-06-07

## Review Inputs

- `agent3` review: no HIGH blockers. Remaining MEDIUM items are deferred risk
  tracking around Gemini native loading, unknown-provider policy defaults,
  route-file maintainability, and role memory error visibility.
- `reviewer1` review: no HIGH blockers. Remaining MEDIUM items are non-blocking
  implementation hygiene notes around filter sweep overlap and seed upgrade
  result semantics.
- `archi` review: one HIGH blocker in `install.sh`.

## HIGH Fixed

`install.sh` previously removed `~/.claude/rules/ccb-config.md` unconditionally
in Claude route-mode install and uninstall cleanup paths. That could delete a
user-authored Claude rules file and violated provider user-home ownership.

The cleanup now removes the external route file only when it contains a known
CCB memory marker:

- `<!-- CCB_CONFIG_START -->`
- `<!-- CCB_ROLES_START -->`
- `<!-- REVIEW_RUBRICS_START -->`
- `<!-- CODEX_REVIEW_START -->`
- `<!-- GEMINI_INSPIRATION_START -->`

Unmarked files are preserved with an explicit message.

## Tests Added

`test/test_install_source_dev_mode.py` now covers:

- route install preserves unmarked external Claude rules file
- route install removes marked CCB external rules file
- uninstall preserves unmarked external Claude rules file
- uninstall removes marked CCB external rules file

## Verification

```bash
bash -n install.sh
pytest -q test/test_install_source_dev_mode.py
pytest -q test/test_install_source_dev_mode.py test/test_project_memory.py test/test_project_memory_filters.py test/test_project_memory_real_context.py test/test_provider_memory_external_context.py test/test_provider_memory_external_matrix.py test/test_provider_core_memory_projection.py test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_v2_runtime_launch.py test/test_v2_tmux_ui.py
git diff --check
python -m py_compile test/test_install_source_dev_mode.py
```

Results:

- `test/test_install_source_dev_mode.py`: 7 passed
- full related suite: 243 passed, 2 skipped
- shell syntax, whitespace, and test-file compile checks passed
