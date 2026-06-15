# Risk Hotspots

Date: 2026-05-25

## README Drift

- `README_zh.md` currently shows version `7.0.2`, but the update section still
  says `CCB v6` and uses `ccb update 6` examples.
- `README.md` includes a clearer windows-topology migration paragraph than
  `README_zh.md`; bilingual parity has already drifted.
- The public README is dominated by a long inline changelog, which pushes
  current v7 operating guidance far below the fold.

## tmux Onboarding Gap

- Current README tmux help is effectively one sentence about copy/paste.
- New users need a practical mental model for CCB-owned tmux, sidebar focus,
  window switching, copy/scroll/paste, detach/reattach, and recovery commands.
- Exact keybinding claims must be verified against CCB's isolated tmux config
  before publishing.

## Media Freshness

- Current media assets are not organized around v7 sidebar/window flows.
- Animations are hidden behind a details block and are not paired with a
  screenshot-by-screenshot operating guide.
- New videos must be captured from sanitized projects to avoid leaking local
  paths, keys, or private prompts.

## Platform Wording

- README badges and install sections mention Linux, macOS, WSL, and Windows.
- Sidebar implementation history contains platform-specific release details.
- README v7 should clearly distinguish core CCB support from sidebar/helper
  support if they differ by platform.

