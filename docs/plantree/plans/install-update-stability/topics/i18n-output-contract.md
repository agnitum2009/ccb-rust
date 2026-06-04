# I18n Output Contract

Date: 2026-06-04

## Objective

CCB currently supports Chinese and English user-facing installer output. The
same expectation must extend to managed update and post-update provisioning.

## Language Selection

Shell installer:

1. `CCB_LANG=zh`, `cn`, or `chinese` selects Chinese.
2. `CCB_LANG=en` or `english` selects English.
3. Otherwise use locale detection from `LANG`, `LC_ALL`, or `LC_MESSAGES`.
4. Fallback is English.

Python update path should use the same contract. The implementation can either
share a small Python i18n helper or mirror the shell keys, but keys and wording
should be reviewed together.

## Must Be Localized

Interactive prompts:

- root install confirmation
- WSL/native environment confirmation
- Role Pack install/update prompt
- Neovim install/update prompt
- newly available Role Pack selection prompt
- major upgrade confirmation

Outcome messages:

- install complete
- update complete
- already up to date
- optional provisioning skipped
- optional provisioning warning
- required provisioning failed
- installed entrypoint smoke failure
- catalog unavailable
- legacy Role Pack migrated
- no work needed/current

Remediation text:

- install Python 3.10+
- install tmux
- retry Role Pack update
- retry Neovim tool install
- choose `agentroles.archi` instead of `ccb.archi`
- run non-interactive skip/force environment variables

## May Stay Stable ASCII

Machine-readable status keys may remain English/ASCII:

- `roles_status: ok`
- `tools_status: ok`
- `install_mode=release`
- `ccbd_state: mounted`

These are diagnostic tokens and tests rely on stable values.

## Style

English:

- concise, action-oriented
- distinguish update success from optional provisioning warnings
- include exact retry command

Chinese:

- direct and operational
- avoid mixing too many English sentence fragments except exact commands,
  role ids, env vars, and paths
- keep prompts short, with the same default as English

## Test Expectations

For every prompt or warning added to install/update:

- one test with `CCB_LANG=en`
- one test with `CCB_LANG=zh`
- one auto-detect locale test when practical
- one non-interactive test showing no prompt is emitted

Tests should assert key wording and the retry command, not full paragraph text
when long explanations would be brittle.
