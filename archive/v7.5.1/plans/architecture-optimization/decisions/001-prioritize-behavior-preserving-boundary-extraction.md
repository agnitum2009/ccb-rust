# Prioritize Behavior-Preserving Boundary Extraction

Date: 2026-05-18

## Context

Architec reports an overall score of `50.11`, with structure at `64.83` and
governance/full at `35.39`. Package topology and coupling control are strong,
but complexity hotspots are concentrated in provider runtime materialization,
storage classification, and the GitHub release checker.

The cleanup inventory also includes many false-positive archive candidates,
including active README/changelog files and authoritative contract documents.

## Decision

Optimize first by extracting behavior-preserving boundaries from high-complexity
files. Do not start with broad file moves, archive actions, or generic provider
abstractions.

The first implementation phase will extract shared provider projection result
and event-recording mechanics while leaving provider-specific auth, config,
session, plugin, and cache semantics in their current modules.

## Consequences

This lowers complexity risk without invalidating the current runtime contracts.
It also keeps the existing test suite useful because public behavior should not
change during the first phase.

Broad cleanup and archive work is deferred until reference searches and owner
decisions prove that a candidate is not active runtime, documentation, or
installer surface.
