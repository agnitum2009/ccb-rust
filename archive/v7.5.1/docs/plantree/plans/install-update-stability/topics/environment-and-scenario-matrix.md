# Environment And Scenario Matrix

Date: 2026-06-04

## Axes

Platform:

- Linux x86_64
- Linux aarch64
- macOS universal
- WSL Linux runtime
- unsupported Windows native

Install mode:

- source/dev checkout
- official release tarball
- preview release tarball
- existing pre-v6 install
- existing v7+ install

User profile:

- normal user
- root
- sudo to root with `SUDO_USER`
- custom `CODEX_INSTALL_PREFIX`
- custom `CODEX_BIN_DIR`
- custom `XDG_DATA_HOME` and `XDG_CACHE_HOME`

Interactivity:

- TTY interactive
- non-interactive CI or pipe
- env-forced install, for example `CCB_INSTALL_ROLES=1`
- env-skipped install, for example `CCB_INSTALL_ROLES=0`

Network:

- GitHub reachable
- GitHub unavailable
- catalog cache already present
- role/tool package source unavailable

Python:

- Python 3.10+ on PATH
- multiple Python versions where `/usr/bin/env python3` is too old
- Python without pip
- externally managed Python requiring `--break-system-packages`
- managed venv enabled
- managed venv disabled

Role state:

- no installed Role Packs
- installed canonical `agentroles.archi` and current catalog digest
- installed canonical `agentroles.archi` and changed catalog digest
- legacy installed `ccb.archi`
- stale legacy `source_path` pointing at removed CCB source-tree roles
- project lock pinned to older installed digest
- catalog unavailable

Tool state:

- no Neovim wrapper
- managed Neovim wrapper present and healthy
- system Neovim present
- LazyVim profile broken
- Architec wrapper present and current
- Architec wrapper missing or old

Language:

- `CCB_LANG=zh`
- `CCB_LANG=en`
- locale auto-detect Chinese
- fallback English

## Required Outcomes

Fresh install:

- Missing required Python or terminal backend fails before mutation where
  possible, with bilingual remediation.
- Optional `tomli`, `watchdog`, Droid, Role Pack, and Neovim failures do not
  fail install unless explicitly required.
- Already installed optional dependencies are reported as ready or current.
- Root install requires explicit confirmation and clearly states root profile
  boundaries.

Managed update:

- Unsupported platform fails early.
- Failed download, extraction, staged installer, or new entrypoint smoke check
  fails the update.
- Post-update optional provisioning can warn but must not make a successful
  core update appear broken.
- Non-interactive update skips optional provisioning and prints exact follow-up
  commands.
- Source/dev update installs the selected release into the managed prefix while
  leaving `./ccb` in the checkout as live source.

Role Pack update:

- `current` Role Packs are not reinstalled or re-run through update hooks.
- `update_available` Role Packs update from the canonical catalog source.
- Legacy `ccb.archi` metadata is migrated or treated as an alias for
  `agentroles.archi`.
- Stale installed `source_path` values do not block catalog fallback.
- Project locks are not changed by install/update/sync unless the user runs an
  explicit adopt/add command.

Language:

- Prompts that ask the user to decide must be available in Chinese and English.
- Warning summaries and follow-up commands must be available in Chinese and
  English.
- Machine-readable tokens can remain stable ASCII, for example
  `roles_status: ok`.
