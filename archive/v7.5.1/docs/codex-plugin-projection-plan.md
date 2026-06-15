# Codex Plugin Projection Plan

## 1. Purpose

This plan defines how `ccb` must project Codex plugin assets into a managed
`CODEX_HOME`.

It closes the architecture gap behind issue `#196`: the managed home inherited
plugin-related config intent, but did not consistently inherit the plugin
catalog and installed plugin assets required to satisfy that intent.

This document complements the authority contract in
[docs/codex-session-isolation-contract.md](/home/bfly/yunwei/ccb_source/docs/codex-session-isolation-contract.md).

## 2. Problem Statement

The broken state was:

- managed `config.toml` could still declare or preserve plugin-enabled behavior
- managed `commands/` and `skills/` could still be projected
- but managed `CODEX_HOME` could start without the plugin marketplace and plugin
  bundle tree that Codex expects under `.tmp/plugins`

That produced an incoherent managed home:

- plugin intent was present
- plugin assets were absent
- startup behavior depended on whether Codex later repopulated cache-like state
  on its own

That is not a runtime cache miss. It is a startup authority mismatch.

## 3. Architectural Decision

Codex plugin projection is startup-owned managed-home authority.

`ccb` must treat these classes separately:

- inheritable startup authority
  - `config.toml`
  - `auth.json`
  - `skills/`
  - `commands/`
  - plugin bundle authority under `.tmp/plugins/`
  - plugin freshness marker under `.tmp/plugins.sha` when present
- non-authoritative runtime residue
  - session logs
  - history and request transcripts
  - provider runtime logs
  - any future provider-generated ephemeral caches outside the plugin bundle
    authority described above

Rejected designs:

- copy all of `~/.codex`
- wait for Codex to lazily heal missing plugin assets after launch
- treat a previously populated managed `.tmp/plugins` tree as sufficient proof
  even when the source plugin bundle changed

## 4. Scope Of Projection

For managed Codex homes, `ccb` must project the source-home plugin authority
root:

- `<source-codex-home>/.tmp/plugins/`
- `<source-codex-home>/.tmp/plugins.sha` when present

That tree is projected as a unit because the marketplace listing, installed
plugin metadata, plugin manifests, bundled commands, bundled skills, bundled
agents, and assets are all internally path-coupled under the same relative
layout.

`ccb` must not attempt to model only a subset such as:

- only `marketplace.json`
- only installed plugin manifests
- only plugin `commands/` or `skills/`

Those subsets recreate the same incoherent-home failure in a different shape.

## 5. Refresh Rules

Startup refresh must be deterministic:

1. If the source plugin tree is absent, remove the managed plugin tree and its
   freshness marker from the managed home.
2. If the source plugin tree is present and the source freshness marker differs
   from the managed one, replace the managed projection.
3. If no source freshness marker exists, `ccb` may fall back to a tree-signature
   comparison, but it must not silently assume the target is current.
4. Refresh must replace the plugin tree as a unit so removed plugins do not
   remain as stale managed residue.

The fast path should use `.tmp/plugins.sha` when available because the plugin
bundle tree can be large and should not be fully recopied on every launch.

## 6. Ownership Boundary

The managed plugin projection belongs to the managed Codex home, not to:

- project runtime logs
- session binding state
- completion detection
- foreground pane ownership

Therefore this fix belongs in the Codex managed-home materialization layer,
not in:

- post-launch recovery hooks
- completion polling
- ad hoc cold-start repair code

## 7. Tests

The regression surface must include:

- provider-profile materialization copies plugin authority into a fresh managed
  home
- explicit API-route managed homes still receive plugin projection
- managed home refresh updates projected plugin assets when the source plugin
  freshness marker changes
- refresh removes stale managed plugin residue when the source projection is no
  longer present
