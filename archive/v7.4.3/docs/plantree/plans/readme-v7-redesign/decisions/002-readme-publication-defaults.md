# README Publication Defaults

Date: 2026-05-25

## Context

The README v7 redesign needs owner decisions before public README and media
work can proceed. The maintainer clarified the target audience, demo source,
update wording, tmux scope, platform wording, and presentation style.

## Decision

- Keep only current v7 highlights in the README and link full history to
  `CHANGELOG.md`.
- Author `README_zh.md` first, then update `README.md` for parity.
- Store new optimized public media under `assets/readme_v7/`; do not commit raw
  recordings in this pass.
- Use a sanitized demo project for screenshots and demo videos.
- Optimize the default reader path for new users, especially users unfamiliar
  with tmux.
- Replace stale v6 update examples with v7/current wording.
- Teach only CCB-required tmux operations.
- Use conservative platform wording: distinguish core CCB support from native
  sidebar/helper support.
- Use collapsible `<details>` sections where they reduce first-read impact,
  while keeping intuitive visual information visible.

## Consequences

- The new README should be shorter above the fold, not an expanded version of
  the old structure.
- The long changelog should stop dominating the README body.
- Advanced configuration remains available but folded.
- Media capture must use sanitized content and optimized committed artifacts.
- Exact diagnostics commands still need verification before publication.
- Later decision
  [003-readme-final-publication-choices.md](003-readme-final-publication-choices.md)
  fixes screenshot style, release-first install wording, and native Windows
  v5-only support wording.
