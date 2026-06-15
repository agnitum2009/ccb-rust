# Use A Task-First v7 README

Date: 2026-05-25

## Context

The current public README files already carry v7 release notes, but the user
journey still reads like an older overview plus a long changelog. v7 introduced
native sidebar control, named windows topology, and more visible project state,
while many users are unfamiliar with tmux.

## Decision

Redesign the README around task-first v7 onboarding:

- show the v7 UI early with fresh media;
- teach first run, daily operation, and safe cleanup before deep history;
- explain sidebar, windows topology, and ask workflows as current product
  behavior;
- include a practical tmux survival guide for non-tmux users;
- keep README_zh.md and README.md in parity.

## Consequences

- The README implementation should not merely add more text to the old
  structure.
- New screenshot and demo-video assets are part of the deliverable, not optional
  polish.
- The long changelog needs an explicit placement decision before the README can
  stay concise.
- Exact tmux key and platform claims must be verified before publication.
