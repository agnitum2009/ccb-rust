# Wave 5: persistent mount ownership

## Goal

Persist MountManager / OwnershipGuard state across daemon restarts so runtime mount ownership is not lost.

## Requirements

- Serialize `MountManager` / `OwnershipGuard` state to durable storage (e.g. `ccbrd` state dir).
- On daemon startup, deserialize and re-establish ownership records.
- Ensure ownership guards survive daemon restart without leaking or duplicating mounts.

## Acceptance Criteria

- [x] After daemon restart, ownership records reflect pre-shutdown state.
- [x] No duplicate ownership guards created on restart.
- [x] Tests cover save/load roundtrip and restart scenario.
- [x] Part of Wave 5 parity audit gap #2.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
