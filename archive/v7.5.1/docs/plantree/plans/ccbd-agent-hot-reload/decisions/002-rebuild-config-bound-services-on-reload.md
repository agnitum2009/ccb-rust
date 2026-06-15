# Rebuild Config-Bound Services On Reload

Date: 2026-05-28

## Context

The current daemon injects the startup config object into registry,
supervisor, supervision, completion tracking, dispatcher, project view, and
project focus services. Updating only `app.config` would leave these services
with stale desired-agent and topology state.

## Decision

Hot reload should rebuild the config-bound service bundle and swap it into the
running app after validation succeeds. Persistent stores, project namespace,
mount manager, lifecycle generation, socket ownership, execution services, and
runtime authority remain shared.

## Consequences

The reload path avoids a fragile web of in-place mutations and keeps a clean
transaction boundary: either every config-bound service observes the new config,
or the daemon continues using the old accepted config. The builder must be
small and testable so startup and reload do not drift.

Related topic:
[current-runtime-boundaries.md](../topics/current-runtime-boundaries.md).
