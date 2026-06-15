# Neovim And LazyVim Provisioning

Date: 2026-05-30

## Goal

`ccb update` and `install.sh install` should be able to prepare a CCB-managed
Neovim/LazyVim tool profile so a project can declare:

```toml
[tool_windows.neovim]
command = "ccb-nvim"
label = "neovim"
```

and get a working Neovim window without requiring the user to manually install
Neovim, clone LazyVim, or tune tmux for common terminal behavior.

The install must be clean: CCB must not overwrite a user's existing Neovim
configuration or global tmux config.

## Source References

- Neovim publishes pre-built release packages for Linux and macOS on its
  GitHub releases page, including Linux AppImage/tarball and macOS arch
  tarballs.
- LazyVim's official installation uses the `LazyVim/starter` template and
  recommends running `:LazyHealth` after first start.
- LazyVim's starter uses `lazy.nvim` and defaults LazyVim/lazy.nvim to their
  latest stable release behavior unless the user changes plugin specs.

## Provisioning Contract

Add a reusable provisioning path shared by source install, release install, and
`ccb update`:

```text
ccb tools doctor neovim
ccb tools install neovim
ccb tools update neovim
```

Installer/update hooks should call the same internal implementation rather than
embedding Neovim-specific shell logic in multiple places.

Default policy:

- interactive install/update asks whether to install or refresh the
  CCB-managed Neovim/LazyVim bundle;
- non-interactive install/update skips provisioning and prints
  `ccb tools install neovim` as the follow-up command;
- failures are warnings by default so CCB itself still installs;
- `CCB_INSTALL_NEOVIM=0` skips it without prompting;
- `CCB_INSTALL_NEOVIM=1` requires provisioning and fails install/update on
  error;
- `CCB_LAZYVIM_PROFILE=0` installs only the Neovim binary wrapper.

## Storage Layout

Do not store mutable Neovim profile state under the release install prefix if
that prefix is replaced on every update.

Preferred stable paths:

```text
~/.local/share/ccb/tools/neovim/
  manifest.json
  bin/nvim -> versioned binary
  versions/<version>/
  lazyvim/profile/
  lazyvim/starter-template/

~/.local/state/ccb/tools/neovim/
  xdg-state/

~/.cache/ccb/tools/neovim/
  xdg-cache/
```

The `ccb-nvim` wrapper should launch with isolated XDG paths:

```text
XDG_CONFIG_HOME=~/.local/share/ccb/tools/neovim/lazyvim/profile/config
XDG_DATA_HOME=~/.local/share/ccb/tools/neovim/lazyvim/profile/share
XDG_STATE_HOME=~/.local/state/ccb/tools/neovim/xdg-state
XDG_CACHE_HOME=~/.cache/ccb/tools/neovim/xdg-cache
NVIM_APPNAME=nvim
```

This makes Neovim read `.../profile/config/nvim` instead of
`~/.config/nvim`, and keeps LazyVim plugin data out of the user's normal
Neovim home.

## Binary Acquisition

Use a versioned manifest checked into CCB, for example:

```json
{
  "neovim_version": "0.11.x",
  "assets": {
    "linux-x86_64": {
      "url": "https://github.com/neovim/neovim/releases/download/...",
      "sha256": "..."
    },
    "macos-arm64": {
      "url": "https://github.com/neovim/neovim/releases/download/...",
      "sha256": "..."
    },
    "macos-x86_64": {
      "url": "https://github.com/neovim/neovim/releases/download/...",
      "sha256": "..."
    }
  },
  "lazyvim_starter_commit": "..."
}
```

Rules:

- Prefer official release tarballs over package-manager installs so CCB has a
  predictable binary path.
- Verify downloaded asset checksums before activation.
- Keep the previously working version until the new version verifies.
- On unsupported platforms, reuse system `nvim` if it satisfies the minimum
  version and report a clear warning otherwise.
- Do not require Rust for this feature.

## LazyVim Profile

LazyVim setup should clone or copy the `LazyVim/starter` template into the
managed profile, then remove `.git` from the managed profile to avoid treating
it as a user project repository.

First implementation note: CCB writes an isolated `init.lua` that bootstraps
`lazy.nvim` and loads `LazyVim/LazyVim` into the managed profile, then runs
`ccb-nvim --headless +Lazy! sync +qa` during provisioning. A fuller
starter-template mirror can be added later without changing the wrapper path or
user config isolation contract.

The managed profile must not assume that the user's terminal font supports Nerd
Font glyphs. CCB writes a managed `lua/plugins/ccb-terminal-compat.lua` overlay
that defaults `mini.icons` and LazyVim UI icon tables to ASCII-safe output.
Users who explicitly want glyphs can launch with
`CCB_LAZYVIM_ICON_STYLE=glyph ccb-nvim`.

Profile update policy:

- first install creates the managed profile;
- update refreshes CCB-owned bootstrap files only when the profile still carries
  the CCB managed marker;
- user-local files under an explicit override folder are preserved;
- destructive profile reset requires an explicit command such as
  `ccb tools reset neovim`.

## tmux Compatibility

Compatibility must be applied to CCB-managed tmux sessions, windows, or panes,
not user-global `~/.tmux.conf`.

First-slice tmux policy:

- set a true-color terminal feature for the CCB session when supported;
- enable focus events at the session level;
- keep mouse behavior aligned with the existing CCB session policy;
- keep escape-time low enough for Neovim key sequences without changing global
  tmux;
- prefer an OSC52 clipboard lane when no platform clipboard helper is present;
- set the tool pane environment so `TERM`, `COLORTERM`, and XDG paths are
  coherent for Neovim.

The exact tmux commands must use the existing project/session scoped tmux
backend helpers so the feature honors CCB's isolation rule.

## Install/Update Flow

`install.sh install`:

1. install CCB as today;
2. ask in an interactive terminal whether to provision Neovim/LazyVim, unless
   `CCB_INSTALL_NEOVIM` forces or skips it;
3. create or refresh the `ccb-nvim` wrapper;
4. print `OK`, `WARN`, or `SKIP` with a short reason.

`ccb update`:

1. update CCB as today;
2. run the same prompt/provisioner after the new package is installed;
3. preserve the last working Neovim profile if the new provisioning step fails;
4. report the tool status in the update summary.

`ccb tools doctor neovim`:

- reports resolved binary path and version;
- reports LazyVim profile path and marker state;
- runs a headless health check and reports degraded when `lazy.nvim` or
  LazyVim plugin files are missing;
- reports tmux compatibility readiness;
- reports missing optional dependencies such as clipboard helpers;
- never mutates state.

## Failure Policy

Provisioning should fail closed for integrity and fail soft for optionality:

- checksum mismatch: do not activate the binary;
- partial download: keep old version;
- LazyVim bootstrap failure: remove partial plugin directories, retry once,
  fall back from `git clone` to the GitHub stable tarball, then warn if both
  fail;
- missing network in interactive/default soft mode: warn and continue CCB
  install/update;
- explicit `CCB_INSTALL_NEOVIM=1`: return non-zero on provisioning failure;
- unsupported OS/arch: skip with a clear diagnostic unless a compatible system
  `nvim` is available.

## Implementation Slices

1. Add `ccb tools doctor/install/update neovim` with fake downloader tests.
2. Add official release metadata lookup, checksum verification, and stable
   storage paths.
3. Add `ccb-nvim` wrapper and isolated XDG LazyVim profile.
4. Integrate provisioning into `install.sh install` and `ccb update`.
5. Wire the Neovim tool-window preset to use `command = "ccb-nvim"`.
6. Add CCB session-scoped tmux compatibility settings before launching the tool
   pane.
