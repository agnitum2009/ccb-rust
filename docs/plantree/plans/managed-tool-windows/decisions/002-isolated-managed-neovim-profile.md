# Isolated Managed Neovim Profile

Date: 2026-05-30

## Context

The Neovim tool-window feature should work after `ccb update` or
`install.sh install`, but the official LazyVim starter installation normally
targets the user's default Neovim config path. CCB users may already have a
personal `~/.config/nvim`, plugin data, state, cache, and tmux settings.

Mutating those global files would violate CCB's isolation expectations and
would make install/update risky.

## Decision

CCB-managed Neovim/LazyVim installs into CCB-owned, isolated paths and launches
through a `ccb-nvim` wrapper that sets XDG/NVIM environment variables.

The wrapper is the command used by managed Neovim tool windows. It may use a
CCB-downloaded Neovim binary or a verified compatible system `nvim`, but it
must not require or modify the user's default Neovim home.

tmux compatibility is applied only to CCB-managed tmux sessions, windows, or
panes.

## Consequences

- `ccb update` and `install.sh install` can prepare Neovim/LazyVim without
  overwriting personal Neovim files.
- CCB can test a deterministic editor environment.
- Users who want their personal Neovim can still configure a tool window with a
  custom command such as `command = "nvim"`.
- LazyVim plugin data may take disk space under CCB-owned data/cache paths.
- The installer needs an explicit doctor/provisioning path and integrity checks
  instead of relying on whatever `nvim` happens to be on `PATH`.
