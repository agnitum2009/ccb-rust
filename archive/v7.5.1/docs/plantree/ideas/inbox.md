# Ideas Inbox

Date: 2026-05-25

## Promoted

- 2026-05-25: Redesign the public README for the v7 release line with new
  screenshots, demo videos, richer operation docs, and tmux onboarding for
  non-tmux users. Promoted to
  [readme-v7-redesign](../plans/readme-v7-redesign/README.md).
- 2026-06-10: Consider a generic external CCB maintenance heartbeat that
  periodically runs bounded agent-health diagnostics, checks configured-agent
  task and communication status, escalates risk, unknown, or unhealthy states
  to `ccb_self` by default, and exits immediately when the project is healthy
  and idle. The heartbeat must remain independent of provider context and must
  not make `ccb_self` a daemon lifecycle authority. Promoted to
  [ccb-maintenance-heartbeat](../plans/ccb-maintenance-heartbeat/README.md).

## Inbox
