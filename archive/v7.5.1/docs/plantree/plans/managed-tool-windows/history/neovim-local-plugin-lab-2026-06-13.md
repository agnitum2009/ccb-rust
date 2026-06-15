# Neovim Local Plugin Lab: 2026-06-13

Date: 2026-06-13

## Scope

This records an isolated local experiment for the second-phase managed Neovim
profile. It did not change production code and did not target `main`.

Lab path:

- `/home/bfly/yunwei/test_ccb2/neovim-enhancement-lab`

Branch observed:

- `fix/pr225-reliability-followup`

## External Research Anchors

- LazyVim Markdown extra:
  <https://lazyvim.github.io/extras/lang/markdown>
- LazyVim Treesitter stack:
  <https://lazyvim.github.io/plugins/treesitter>
- nvim-treesitter main README:
  <https://github.com/nvim-treesitter/nvim-treesitter>
- Neovim Treesitter help:
  <https://neovim.io/doc/user/treesitter/>
- render-markdown.nvim:
  <https://github.com/MeanderingProgrammer/render-markdown.nvim>
- Markview.nvim:
  <https://github.com/OXY2DEV/markview.nvim>
- Snacks image docs:
  <https://github.com/folke/snacks.nvim/blob/main/docs/image.md>
- markdown-preview.nvim:
  <https://github.com/iamcco/markdown-preview.nvim>
- Glow.nvim archived status:
  <https://github.com/ellisonleao/glow.nvim>
- Kitty terminal graphics ecosystem:
  <https://sw.kovidgoyal.net/kitty/integrations/>
- Snacks image tmux/WezTerm issue evidence:
  <https://github.com/folke/snacks.nvim/issues/2165>

Relevant upstream findings:

- `render-markdown.nvim` is the stronger in-buffer Markdown target, but it
  requires Treesitter `markdown` and `markdown_inline` parsers.
- Current `nvim-treesitter` main requires Neovim 0.12, `tar`, `curl`,
  `tree-sitter-cli >= 0.26.1`, and a C compiler for parser installation.
- Snacks image requires terminal graphics support for inline display and
  ImageMagick for non-PNG conversion paths.
- `markdown-preview.nvim` offers browser preview and local image support, but
  it is a heavier optional path because it builds a Node application.
- `glow.nvim` is archived and should not be selected for a new default profile.

## Local Environment

- Linux x86_64 on Ubuntu kernel `6.8.0-90-generic`.
- Current session is not WSL: `WSL_DISTRO_NAME` and `WSL_INTEROP` were empty.
- CCB-managed tmux with `TERM=tmux-256color`.
- Neovim binary: `/usr/bin/nvim`, observed as `NVIM v0.12.0-dev`.
- `node v22.20.0` and `npm 10.9.3` are present.
- `xdg-open` and `xclip` are present.
- `wl-copy`, `pbcopy`, `clip.exe`, `win32yank.exe`, `wslview`, and
  `explorer.exe` were not present in this non-WSL run.
- Inotify limits: `max_user_instances=128`,
  `max_user_watches=524288`, `ulimit -n=1048576`.

## Installed Lab Plugins

The lab reused the managed `lazy.nvim` path and installed plugins into isolated
XDG paths. Plugins tested:

- `folke/snacks.nvim`
- `MeanderingProgrammer/render-markdown.nvim`
- `OXY2DEV/markview.nvim`
- `stevearc/oil.nvim`
- `nvim-mini/mini.files`
- `nvim-neo-tree/neo-tree.nvim`
- `HakonHarnes/img-clip.nvim`
- `nvim-treesitter/nvim-treesitter`

All plugin `require()` checks succeeded after `Lazy! sync`.

## Folder Results

- `Snacks` explorer works as the lowest-friction default for `ccb-nvim <dir>`.
- With default watcher behavior, opening a directory emitted an `EMFILE: too
  many open files` watcher error even though the normal open-file limit was
  high. The machine has only `128` inotify user instances.
- Setting `picker.sources.explorer.watch = false` removed the `EMFILE` error
  while still opening the directory into `snacks_picker_list`.
- `oil.nvim`, `mini.files`, and `neo-tree.nvim` loaded and opened folders, but
  they create additional primary file-management models.

Implication:

- Keep Snacks explorer/picker as the default, but disable watcher behavior in
  the CCB-managed default unless a future doctor check proves it is safe.

## Markdown Results

Baseline lab behavior:

- `render-markdown.nvim` loaded and registered `:RenderMarkdown`, but opening a
  Markdown file failed when the `markdown` parser was unavailable.
- `Markview.nvim` loaded alone and opened the same Markdown file without
  parser-related startup errors.

Parser path finding:

- Clean `nvim --clean` finds system parsers under
  `/usr/lib/x86_64-linux-gnu/nvim/parser/markdown.so` and
  `/usr/lib/x86_64-linux-gnu/nvim/parser/markdown_inline.so`.
- The lazy.nvim-based isolated profile dropped `/usr/lib/x86_64-linux-gnu/nvim`
  from `runtimepath`, so the same installed parsers became invisible.
- Appending `/usr/lib/x86_64-linux-gnu/nvim` after `lazy.setup()` restored
  parser visibility. After that, `render-markdown.nvim` opened the Markdown
  sample without errors.

Parser installation toolchain:

- The old `:TSInstallSync` path was not available with the current
  nvim-treesitter main branch.
- The documented Lua install API needs `tree-sitter-cli >= 0.26.1`.
- Installing `tree-sitter-cli` through npm produced a binary requiring
  `GLIBC_2.39`, which does not run on this Ubuntu 22.04 system.
- Building `tree-sitter-cli 0.26.9` through cargo failed because `libclang.so`
  was not installed.

Implication:

- The default managed profile should first preserve system parser runtime
  paths. It must not assume parser compilation is available.
- `doctor` must use read-only parser checks such as runtime-file lookup and
  `vim.treesitter.language.inspect`; it must not call `vim.treesitter.start()`
  because that can trigger plugin repair/install behavior.
- `render-markdown.nvim` is acceptable only after parser readiness is visible.
  `Markview.nvim` remains a viable fallback candidate if parser readiness
  cannot be made reliable on a platform.

## Browser Preview Results

`markdown-preview.nvim` installed and registered:

- `:MarkdownPreview`
- `:MarkdownPreviewStop`
- `:MarkdownPreviewToggle`

Build observations:

- The build completed with the local Node/npm toolchain.
- `npm install` reported dependency vulnerabilities.
- The build left local changes inside the plugin checkout:
  `app/yarn.lock` modified and `app/package-lock.json` added.

Implication:

- Browser preview should not be installed by default in the first enhanced
  profile. It should remain an optional, capability-gated overlay with explicit
  Node/browser/opener diagnostics.

## Image Results

- `snacks.image.supports_file()` returned true for PNG/JPG/PDF-style filenames
  in the lab.
- `snacks.image.supports_terminal()` returned false in this CCB tmux session.
- A Snacks-only profile opened a PNG sample without startup errors, but inline
  display was unavailable.
- When `render-markdown.nvim` was loaded without visible Markdown parsers,
  opening a PNG triggered the same Markdown parser error because Snacks image
  uses a markdown-like buffer path.
- After restoring the system parser runtime path, opening the PNG no longer
  triggered that error.

Implication:

- Image support depends on both terminal-image capability and Markdown parser
  health.
- Inline images should be degraded by default in current Linux/tmux evidence,
  with external open/reveal commands available as fallback.

## Clipboard And Opener Results

- Neovim exposes `vim.ui.open()` and `vim.system()`.
- Linux opener capability exists through `xdg-open`.
- Linux clipboard helper capability exists through `xclip`, but the managed
  profile's `clipboard` option is currently empty.

Implication:

- CCB should add opener and clipboard capability reporting before binding rich
  keymaps that assume platform integration.

## Resulting Direction

1. Fix/preserve parser runtime paths before enabling rich Markdown or image
   defaults.
2. Use read-only diagnostics for parser, opener, clipboard, image terminal, and
   Node/browser readiness.
3. Default folder workflow: Snacks explorer/picker with watcher disabled.
4. Default Markdown workflow: `render-markdown.nvim` only when parser readiness
   passes; otherwise degrade cleanly and consider Markview as fallback.
5. Default image workflow: Snacks image only when terminal support passes;
   otherwise use external open/reveal fallback.
6. Browser preview: optional overlay, not first default slice.
