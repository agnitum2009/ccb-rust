# Managed Neovim Enhancement Defaults

Date: 2026-06-13

## Context

The first managed Neovim slice proved that CCB can install and launch an
isolated LazyVim profile. The next phase needs the profile to be useful as a
project editor across Linux, macOS, and WSL: open folders, render Markdown
well, handle image references, and integrate with system open/clipboard paths
without breaking startup when optional dependencies are absent.

Local Linux/tmux testing on 2026-06-13 found that advanced features are tightly
coupled to runtime paths, terminal capability, and external helper availability.

## Decision

The managed default profile will use capability-gated enhancements instead of
assuming rich media support.

Defaults for the next implementation slice:

- Preserve system parser runtime paths before enabling Markdown or image
  features that rely on Treesitter parsers.
- Use Snacks explorer/picker as the default folder workflow.
- Disable Snacks explorer watcher behavior by default until doctor can prove
  the environment has enough inotify capacity and watcher support.
- Prefer `render-markdown.nvim` for in-buffer Markdown once
  `markdown`/`markdown_inline` parser readiness is visible.
- Keep `markdown-preview.nvim` out of the default profile; expose browser
  preview later as an optional overlay with Node/browser/opener diagnostics.
- Use `snacks.image` only behind terminal-image capability checks, and fall
  back to external open/reveal behavior when inline rendering is unavailable.
- Keep paste-image and clipboard integration optional until doctor can report
  platform-specific clipboard readiness.
- Use a CCB-owned managed-profile lockfile or equivalent pinning policy before
  shipping the enhanced plugin set, because LazyVim/nvim-treesitter/plugin drift
  changes runtime and parser requirements.

`ccb tools doctor neovim` must report these surfaces through read-only probes.
It must not attempt parser installation, plugin repair, or browser startup
while diagnosing capability.

## Consequences

- `ccb-nvim .` should become reliable without introducing multiple competing
  file managers.
- Markdown becomes useful in-terminal when parser readiness is present, and
  missing parsers become a degraded capability rather than a startup error.
- Image support becomes honest: inline where terminal/tmux support is real,
  external fallback elsewhere.
- Browser preview remains available to plan for, but does not add Node/browser
  risk to the default install path.
- The implementation must add tests for runtimepath preservation, read-only
  parser detection, watcher-disabled directory opening, and degraded image
  behavior.
