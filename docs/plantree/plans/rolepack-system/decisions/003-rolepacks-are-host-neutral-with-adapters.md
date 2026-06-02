# Role Packs Are Host Neutral With Adapters

Date: 2026-06-01

## Context

The roles concept should grow beyond CCB. If the manifest directly assumes
tmux, `.ccb`, Codex home paths, or CCB reload behavior, other hosts cannot
reuse the package. At the same time, CCB needs concrete adapter behavior for
config, projection, and diagnostics.

## Decision

Role Packs have a host-neutral core manifest and optional host/provider
adapters. The core defines identity, responsibilities, assets, compatibility,
permissions, and tool lifecycle hooks. CCB-specific behavior belongs in CCB
adapter fields or files.

## Consequences

- Other hosts can implement the same Role Pack model.
- CCB can still provide first-class commands and projection behavior.
- Provider-specific skill formats remain isolated under provider directories.
- The spec must clearly separate core required fields from adapter-specific
  extensions.

