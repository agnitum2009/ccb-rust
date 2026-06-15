# Managed Neovim System Optimization

Date: 2026-06-12

Related:

- [../roadmap.md](../roadmap.md)
- [neovim-lazyvim-provisioning.md](neovim-lazyvim-provisioning.md)
- [test-matrix.md](test-matrix.md)
- [../../../../ccb-wsl-compatibility-plan.md](../../../../ccb-wsl-compatibility-plan.md)
- [../history/neovim-local-plugin-lab-2026-06-13.md](../history/neovim-local-plugin-lab-2026-06-13.md)
- [../decisions/003-neovim-enhancement-defaults.md](../decisions/003-neovim-enhancement-defaults.md)

## Purpose

This is the second-phase plan for the CCB-managed Neovim tool window. The
first phase made `ccb-nvim` installable, isolated, and safe. This phase should
make it a useful cross-platform editor profile across Linux, macOS, and WSL
without weakening the tool-window and profile-isolation decisions.

## Current Inventory

Implemented behavior observed in source:

- `lib/cli/tools_runtime/neovim.py` creates an isolated `ccb-nvim` wrapper with
  CCB-owned XDG paths and `NVIM_APPNAME=nvim`.
- The wrapper uses a managed Neovim binary when no system `nvim` exists, or a
  system `nvim` when available.
- Managed binary download currently maps Linux x86_64/aarch64 and macOS
  arm64/x86_64 to official Neovim release tarballs and verifies sha256 before
  activation.
- The managed LazyVim profile writes a CCB-owned `init.lua` and
  `lua/plugins/ccb-terminal-compat.lua`.
- The compatibility overlay defaults icon-heavy LazyVim surfaces to ASCII-safe
  output unless `CCB_LAZYVIM_ICON_STYLE=glyph` is set.
- Tests cover isolation from `~/.config/nvim`, LazyVim repair and tarball
  fallback, missing-network degradation, Linux x86_64 managed binary download,
  checksum mismatch, and doctor routing.

Gaps for this phase:

- WSL is not modeled as a distinct Neovim capability surface.
- `ccb tools doctor neovim` does not yet report clipboard, opener,
  terminal-image, WSL, ImageMagick, browser preview, or mounted-drive
  performance risks.
- The default profile does not yet define a stable CCB contract for directory
  browsing, Markdown rendering/preview, image viewing, pasted images, or
  external file opening.
- Rich features are terminal and dependency sensitive; enabling them blindly
  would create broken first-run behavior on some Linux, macOS, tmux, and WSL
  setups.
- The current lazy.nvim-based isolated profile can drop distro parser runtime
  paths, making installed system Treesitter parsers invisible to Markdown and
  image plugins.
- Plugin drift remains possible because the CCB profile follows upstream
  LazyVim/lazy.nvim behavior rather than a CCB-owned lockfile.

## Capability Check: 2026-06-13 Linux/X11/tmux

Environment:

- Source validation wrapper:
  `/home/bfly/yunwei/ccb_source/ccb_test --diagnose` from
  `/home/bfly/yunwei/test_ccb2` reports the source wrapper is active and the
  test project is outside the source checkout.
- Platform: Linux x86_64, X11, inside CCB-managed tmux with
  `TERM=tmux-256color` and `COLORTERM=truecolor`.
- Not WSL in this run: `WSL_DISTRO_NAME` and `WSL_INTEROP` were empty.

Observed source-side doctor with isolated source home:

- `HOME=/home/bfly/yunwei/test_ccb2/source_home`
  `CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home`
  `/home/bfly/yunwei/ccb_source/ccb_test tools doctor neovim`
  reported `neovim_status: missing`.
- The same doctor found `/usr/bin/nvim`, but no isolated `ccb-nvim` wrapper in
  that source test home.

Observed installed user profile:

- `ccb tools doctor neovim` reported `neovim_status: ok`,
  `lazyvim_sync_status: ok`, and `lazyvim_health_status: ok`.
- The wrapper launches `/usr/bin/nvim`, observed as `NVIM v0.12.0-dev`.
- The generated managed profile currently contains only:
  - `init.lua`;
  - `lazy-lock.json`;
  - `lazyvim.json`;
  - `lua/plugins/ccb-terminal-compat.lua`.
- Installed plugin directories include `LazyVim`, `snacks.nvim`,
  `nvim-treesitter`, `mini.icons`, and other LazyVim base plugins.
- Installed plugin directories do not include `render-markdown.nvim`,
  `markdown-preview.nvim`, `img-clip.nvim`, `image.nvim`, `oil.nvim`, or a
  CCB-specific open/files/markdown/images/clipboard overlay.

Folder capability:

- `require("snacks")` succeeds.
- `snacks.explorer`, `snacks.image`, and `snacks.picker` are present.
- Opening `/home/bfly/yunwei/test_ccb2` with `ccb-nvim --headless <dir>`
  landed in a `snacks_picker_list` buffer. This confirms the current profile
  has basic Snacks directory handling, but doctor does not report this surface.

Markdown capability:

- Markdown filetype detection works for `.md`.
- `render-markdown.nvim` is not installed, so the stronger in-buffer Markdown
  rendering target is absent.
- A direct Treesitter parser start for `markdown` and `markdown_inline` failed
  in this profile. The probe emitted nvim-treesitter parser download messages,
  which means naive headless checks can mutate or attempt to repair the
  profile.
- Future doctor checks for parser capability must be explicitly read-only, or
  clearly separated from an install/repair command.

Image capability:

- `snacks.image.supports_file()` reports `true` for PNG, JPG, and PDF filename
  extensions in the installed Snacks config, but terminal support is the
  limiting factor.
- `snacks.image.supports_terminal()` returned `false` in this tmux environment.
- Snacks detected the current image terminal environment as `tmux`; Kitty,
  Ghostty, and WezTerm candidates were not detected through the current tmux
  session even though `wezterm` exists on `PATH`.
- `convert`, `gs`, `pdflatex`, and `wezterm` exist. `magick`, `kitty`,
  `ghostty`, `tectonic`, and `mmdc` are missing.
- In this environment, inline images should be marked degraded/unavailable and
  image paths should fall back to external open or reveal-in-explorer.

External opener and clipboard capability:

- Neovim has `vim.ui.open()` and `vim.system()`.
- `xdg-open` is available. `wslview` and `explorer.exe` are not available in
  this non-WSL run.
- `xclip` is available. `wl-copy`, `wl-paste`, `pbcopy`, `pbpaste`,
  `clip.exe`, and `win32yank.exe` are not available.
- Neovim `clipboard` option was empty in the current CCB profile, so the
  profile does not explicitly wire system clipboard behavior even though a
  Linux helper exists.

Immediate implications:

- Directory opening is already good enough to keep Snacks as the default
  baseline, but doctor should expose it and the CCB default should avoid
  watcher pressure unless capacity is known.
- Markdown is not yet good enough for the user's target. Add
  `render-markdown.nvim` only after parser runtime paths and read-only parser
  readiness reporting are fixed.
- Inline images must be capability-gated. Current Linux/tmux evidence supports
  external fallback as the default behavior when terminal detection fails.
- Capability probes must not accidentally trigger parser/plugin installation
  during `doctor`; read-only detection needs its own implementation path.

## Local Plugin Lab Implications

Detailed evidence is recorded in
[../history/neovim-local-plugin-lab-2026-06-13.md](../history/neovim-local-plugin-lab-2026-06-13.md).

Key updates from the isolated lab:

- Clean `/usr/bin/nvim --clean` can see distro parsers under
  `/usr/lib/x86_64-linux-gnu/nvim`, but the lazy.nvim-managed isolated profile
  dropped that path from `runtimepath`.
- Restoring the system parser runtime path made `markdown` and
  `markdown_inline` visible again and stopped `render-markdown.nvim` from
  failing on Markdown open.
- The same parser fix also matters for image handling because Snacks image can
  use a Markdown-like buffer path when opening images.
- Snacks explorer works as the folder default, but watcher-enabled explorer can
  hit inotify `EMFILE` in the current tmux environment. `watch = false` solved
  the observed failure.
- `markdown-preview.nvim` installs and registers commands when Node/npm are
  present, but its build adds heavier browser/Node assumptions and left npm
  generated files in the plugin checkout. Keep it optional.
- Installing or building a fresh `tree-sitter-cli` is not a safe default repair
  path: npm's current binary required a newer glibc on this host, and cargo
  build failed without `libclang.so`.

This changes the first implementation priority: preserve parser runtime paths
and add read-only diagnostics before enabling the richer Markdown/image overlay.

## Goals

- Preserve the existing isolation contract: no writes to user
  `~/.config/nvim`, default Neovim data/cache/state, or global tmux config.
- Make `ccb-nvim` a predictable editing tool window for common project work:
  open folders, inspect files, write Markdown, view Markdown enough to edit,
  open images or image references when the platform supports it, and open
  files/URLs through the system handler.
- Treat cross-platform behavior as capability-gated. Unsupported capabilities
  should show as `skipped` or `degraded` in doctor output, not as broken Neovim
  startup.
- Keep tool windows out of ask routing, provider runtime, Comms, completion
  tracking, and agent health authority.
- Make source validation repeatable through the existing
  `/home/bfly/yunwei/ccb_source/ccb_test` plus `/home/bfly/yunwei/test_ccb2`
  discipline.

## User Intent Update

The default managed profile should be strong enough to serve as a real project
editor, not only a proof that LazyVim starts. At minimum it must open folders
well and make Markdown and image-heavy project files easier to inspect.

This shifts the target from "optional Neovim tool window" to "safe default
project editor profile with capability-gated rich features."

## Non-Goals

- Native Windows support outside WSL.
- Replacing a user's personal Neovim configuration.
- Turning the Neovim tool window into an agent or ask target.
- Requiring Nerd Fonts, Node.js, browser tooling, ImageMagick, or a specific
  terminal for the profile to start.
- Bundling every plugin or every Neovim release binary directly into CCB
  release artifacts.

## Compatibility Model

### Core Editor

This layer must work everywhere CCB supports the Neovim tool:

- compatible `nvim` binary resolution;
- isolated XDG paths;
- LazyVim bootstrap and health check;
- true-color environment;
- tmux focus and escape-time compatibility;
- ASCII-safe UI by default.

### OS Integration

This layer is platform dependent and should be diagnosed separately:

- system opener for files and URLs:
  - Linux: `xdg-open` or desktop-specific equivalent;
  - macOS: `open`;
  - WSL: `wslview`, `explorer.exe`, or another explicit bridge;
- clipboard:
  - Linux X11/Wayland helpers such as `xclip` or `wl-clipboard`;
  - macOS helpers such as `pbcopy`/`pbpaste` and plugin-specific tools;
  - WSL helpers such as Linux clipboard tools, OSC52, `clip.exe`, or an
    explicitly configured bridge.

### Rich Media

This layer should never be assumed:

- terminal image protocol support varies by terminal and tmux passthrough;
- ImageMagick is needed for many non-PNG conversions;
- inline media can fail in WSL, nested tmux, SSH, or terminals without image
  protocol support;
- Markdown browser preview may need a browser, Node-based build steps, or a
  desktop opener.

## Proposed Profile Shape

Keep the current managed `init.lua`, but split the generated overlay into
small CCB-owned plugin modules so doctor output and tests can reason about each
surface:

- `ccb-terminal-compat.lua`: existing ASCII icons, fillchars, true-color, and
  terminal-safe LazyVim defaults.
- `ccb-open.lua`: keymaps and helpers around Neovim's system open behavior,
  with WSL-specific opener selection handled by CCB capability detection.
- `ccb-files.lua`: one default folder workflow. Prefer a single default file
  manager path instead of enabling multiple competing explorers.
- `ccb-markdown.lua`: in-buffer Markdown rendering as the default, with browser
  preview as optional capability-gated functionality.
- `ccb-images.lua`: inline/image preview only when terminal protocol and
  conversion dependencies pass capability checks.
- `ccb-clipboard.lua`: paste image and clipboard helper integration only when
  platform support is explicit.

Candidate defaults to evaluate:

- Folder workflow: prefer `snacks.nvim` explorer for the CCB default because it
  is already part of the LazyVim/Snacks direction, can replace netrw for
  directory arguments, and provides file operations without adding another
  primary explorer. Disable watcher behavior in the managed default until
  doctor can prove watcher capacity. Keep `oil.nvim` as a future optional
  power-user overlay rather than the default.
- Markdown workflow: enable in-buffer rendering by default with
  `render-markdown.nvim` only after parser runtime paths and
  `markdown`/`markdown_inline` parser readiness pass. Browser preview remains
  optional and capability-gated because it adds heavier browser/runtime
  assumptions. Markview remains a fallback candidate when parser readiness
  cannot be guaranteed.
- Image workflow: use `snacks.image` as the default inline image path when its
  own terminal/file support checks pass and parser readiness is healthy, with a
  clean fallback to external open through the platform opener.
- Image paste workflow: keep paste-from-clipboard optional until clipboard
  helper detection is reliable across Linux, macOS, tmux, and WSL.

## Recommended Baseline Stack

Folder and project navigation:

- Enable Snacks explorer and picker in the generated CCB profile.
- Set `picker.sources.explorer.watch = false` in the managed default unless a
  future doctor check reports watcher support as healthy.
- Set explorer replacement for netrw so `ccb-nvim .` and opening a directory
  inside Neovim produce an explorer instead of an empty or confusing buffer.
- Keep keypaths aligned with LazyVim conventions:
  - `<leader>e`: project/root explorer;
  - `<leader>E`: current working directory explorer;
  - `<leader><space>` or existing LazyVim picker key: find files.
- Doctor should verify that opening a directory does not fall back to netrw or
  fail before the profile is considered fully healthy.

Markdown:

- Add `MeanderingProgrammer/render-markdown.nvim` to the managed profile and
  configure it with ASCII-safe headings/checkmarks when
  `CCB_LAZYVIM_ICON_STYLE` is not `glyph`.
- Preserve system parser runtime paths before checking or enabling
  `render-markdown.nvim`.
- Ensure Treesitter parsers needed for Markdown rendering are visible or
  clearly reported as degraded. The profile should remain usable even if parser
  installation or compilation is not available.
- Keep `markdown-preview.nvim` out of the first default slice. Add it later
  only when doctor can report browser/opener/runtime readiness.

Images:

- Enable `snacks.image` only behind capability checks:
  - terminal supports the Kitty graphics protocol path used by Snacks;
  - tmux passthrough is available or the session is not inside tmux;
  - ImageMagick is available when non-PNG conversion is needed.
- For unsupported terminals or WSL cases, provide explicit fallback commands:
  - open image or Markdown image target externally;
  - reveal the path in the explorer;
  - report why inline rendering is unavailable.
- Avoid treating inline image failure as LazyVim profile failure. It should
  degrade the image surface only.

External open:

- Use Neovim's system-open behavior as the common path, but let CCB doctor
  diagnose the effective opener:
  - Linux: `xdg-open`;
  - macOS: `open`;
  - WSL: `wslview`, `explorer.exe`, or configured fallback.
- Add a managed keymap for opening the current file and for opening a file/URL
  under cursor, while keeping normal editing behavior intact.

## Why Not Default To Oil First

`oil.nvim` is a good project and remains a strong option, but it creates a
second primary file-management model beside LazyVim/Snacks. For a managed
default profile, the lower-risk path is to make the existing LazyVim/Snacks
stack coherent before adding another explorer. If users later need
buffer-style filesystem editing, it can be added as an optional overlay without
changing the default folder-open contract.

## Implementation Slices

1. Parser runtime and read-only diagnostics foundation:
   - preserve distro/system parser runtime paths after lazy.nvim setup;
   - add read-only parser probes for `markdown` and `markdown_inline`;
   - ensure `doctor` does not call checks that start parser installation,
     plugin repair, or browser processes.
2. Capability diagnostics expansion:
   - detect platform, WSL, terminal program, tmux passthrough readiness,
     opener, clipboard helper, ImageMagick, and optional browser/Node support;
   - add machine-readable keys to `ccb tools doctor neovim`, for example
     `opener_status`, `clipboard_status`, `image_status`,
     `markdown_preview_status`, and `wsl_status`;
   - add unit tests with mocked platform/env/path states before enabling new
     profile features.
3. Profile overlay modularization:
   - split the generated compatibility overlay into stable CCB-owned modules;
   - keep existing ASCII behavior and marker checks;
   - preserve user override paths and never overwrite non-managed files.
4. Folder baseline:
   - enable Snacks explorer/picker as the default directory workflow;
   - disable Snacks explorer watcher behavior by default;
   - verify `ccb-nvim .`, opening a directory path, and `<leader>e` open folders
     predictably inside the tool window.
5. Markdown baseline:
   - enable in-buffer Markdown viewing with `render-markdown.nvim` when
     parser/plugin state is healthy;
   - degrade cleanly, and evaluate Markview fallback, when parser readiness is
     unavailable;
   - keep browser preview optional and diagnosable.
6. Image and media baseline:
   - gate `snacks.image` inline support on terminal protocol, tmux passthrough,
     parser readiness, and conversion helper readiness;
   - fall back to external open when inline rendering is unavailable.
7. WSL-specific behavior:
   - detect WSL separately from generic Linux;
   - warn when tool data/cache/state are placed on a mounted Windows drive if
     that causes known performance or execution problems;
   - choose an opener and clipboard fallback policy that aligns with the WSL
     compatibility plan's artifact/authority separation.
8. Validation and rollout:
   - expand unit tests for Linux, macOS, WSL, missing helper, and helper-present
     capability states;
   - run live validation from `/home/bfly/yunwei/test_ccb2`;
   - record Linux, macOS, WSL home, and WSL `/mnt/<drive>` manual results in
     the test matrix or issue log.

## Acceptance Criteria

- `ccb tools doctor neovim` explains which advanced surfaces are available,
  skipped, or degraded without mutating state.
- `ccb tools install neovim` remains optional by default and required only when
  `CCB_INSTALL_NEOVIM=1`.
- A fresh managed profile can open a project folder and edit normal source
  files on Linux, macOS, WSL home, and WSL mounted-drive projects.
- Markdown files are readable in the managed profile without requiring a
  browser.
- Installed system Treesitter parsers remain visible from the isolated managed
  profile when the underlying Neovim distribution provides them.
- Image references either render inline when capabilities pass or open through
  a diagnosed external fallback.
- Missing image, clipboard, browser, or terminal-image dependencies do not
  break Neovim startup.
- No test or live validation writes user `~/.config/nvim`, default Neovim
  data/cache/state, or global `~/.tmux.conf`.
- Tool-window add/remove reload still leaves unrelated agent panes and `ccb ask`
  routing intact.

## Risks

- LazyVim and plugin upstream drift can break generated overlay assumptions.
- Lazy/lazy.nvim runtimepath optimization can hide distro parser directories if
  CCB does not preserve them explicitly.
- Terminal image protocol support differs across Kitty, Ghostty, WezTerm, tmux,
  SSH, and WSL terminals.
- WSL may have multiple plausible clipboard/opener paths, and choosing the
  wrong one can make behavior appear flaky.
- Rich Markdown preview can pull in browser and Node dependencies that are too
  heavy for a default profile.
- Mounted Windows drives can be slower or have different execution semantics,
  even when the Neovim profile itself lives under a Linux XDG root.

## Readiness

Ready to implement the diagnostics/runtimepath/folder foundation, but not ready
to enable every rich media surface by default. Remaining gates are:

- WSL opener and clipboard fallback order;
- macOS and WSL manual validation;
- final policy for automatic inline image attempts versus explicit commands;
- Markview fallback behavior when parser readiness is unavailable.
