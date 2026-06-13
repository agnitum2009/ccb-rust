# README Final Publication Choices

Date: 2026-06-12

## Context

The README rewrite needed final maintainer choices for screenshot style,
installation/update positioning, and platform support wording before drafting
the public `README_zh.md` patch. On 2026-06-12, the maintainer changed the
default new-install recommendation from GitHub release packages to npm.

## Decision

- Regenerate real terminal screenshots for the public README before merging.
  The current `ccb_test2` text-rendered annotated images remain planning
  references and may guide annotations, but they are not the final hero media.
- Use the existing dark terminal visual style for regenerated screenshots.
  Unless a later branding pass changes it, use a wide README-friendly terminal
  size and keep annotations sparse.
- Explain each important visible area in the final screenshot: sidebar, window
  list, agent rows, active marker, Comms, agent panes, pane title/border, and
  basic functions.
- Use npm as the default recommended new-install path in the README:
  `npm install -g @seemseam/ccb` for new installs. After CCB is installed,
  subsequent updates should use `ccb update`. GitHub release packages remain
  available when npm is unavailable. Source checkout install remains available
  for development or fallback guidance, but it should not be the primary
  new-user path.
- Document platform support conservatively: current/new v7 README guidance
  should not claim native Windows support. Native Windows support only applies
  to the v5 line; newer versions are not supported natively on Windows. For
  Windows users on new versions, recommend WSL where applicable.

## Consequences

- `assets/readme_v7/` should receive polished terminal raster screenshots before
  the README patch is finalized.
- Screenshot production can proceed without further owner input: use dark theme,
  wide terminal framing, and sparse numbered annotations with detailed
  explanations in README tables.
- README install sections should be rewritten around npm-first new-install
  language plus `ccb update` for later updates, with GitHub release package and
  source/dev install folded or clearly marked as fallback/development paths.
- Platform badges and platform notes must distinguish current Unix-like/WSL
  support from legacy native Windows v5 support.
- Any old README wording that implies v7 native Windows support must be removed
  or rewritten.
