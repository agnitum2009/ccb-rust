# Fix ccb-cli flaky environment tests

## Goal

source_guard + doctor_runtime tests depend on process-global env vars (CCB_SOURCE_ALLOWED_ROOTS, uid/ownership) and race under Rust's default parallel test runner -> flaky failures. Serialize env-dependent tests via static Mutex (no new deps).

## Requirements

- TBD

## Acceptance Criteria

- [ ] TBD

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
