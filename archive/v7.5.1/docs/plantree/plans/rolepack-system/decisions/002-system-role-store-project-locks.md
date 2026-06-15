# System Role Store With Project Locks

Date: 2026-06-01

## Context

Copying role assets into every project would duplicate skills, prompts, memory
templates, and external tool wrappers. However, sharing provider sessions,
auth, runtime authority, or agent private memory would break isolation.

## Decision

Role assets are installed once into a system role store. Projects record role
references and exact locks. Agent provider homes receive projected assets from
the system role store, while sessions and runtime state remain agent-private.

## Consequences

- Role assets can be updated and cached centrally.
- Projects remain small and contain only role references, locks, and overrides.
- Provider sessions, auth, and runtime authority remain isolated.
- Projection needs markers or symlink provenance so cleanup can distinguish
  role-owned assets from user-authored provider files.
- Project locks prevent silent version drift when the system role store updates.

