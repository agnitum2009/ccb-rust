# Wave 5: supervision loop recovery

## Goal

Implement RuntimeSupervisionLoop end-to-end recovery orchestration instead of the current stub tick.

## Requirements

- Replace stub `SupervisionLoop::tick` with real health assessment per agent.
- Detect pane-dead / pane-missing / session-missing / unhealthy states.
- Trigger recovery actions: respawn pane, restart provider, escalate after max retries.
- Integrate with backoff and restart-count tracking.

## Acceptance Criteria

- [ ] Supervision loop uses `health_assessment::assess_provider_pane` or equivalent to decide restart.
- [ ] Dead pane triggers respawn; no restart storm due to backoff.
- [ ] Unit tests cover healthy/dead/missing pane scenarios.
- [ ] Part of Wave 5 parity audit gap #3.

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
