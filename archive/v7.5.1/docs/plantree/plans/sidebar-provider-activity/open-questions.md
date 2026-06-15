# Open Questions

Date: 2026-05-27

## Codex

- Does the current Codex managed-home hook configuration require an explicit
  trust record, or is projected managed config sufficient?
- Which Codex failure event should map to `failed` versus recoverable
  `pending` when a turn is interrupted or blocked?
- Should Codex app-server be evaluated immediately after hook activity lands,
  or only after sidebar false-idle is fixed?

## Claude

- Which Claude `Notification` payloads mean "waiting for user" rather than
  ordinary idle notification?
- Can Claude activity status be sourced entirely from hooks, or does sidebar
  failure detection also need a bounded session-log watcher?
- How long should `Stop` with running background tasks keep the agent active
  before freshness fallback takes over?

## Testing

- Which local fault proxy should be the standard manual test tool for API
  disconnect and stream-cut scenarios?
- Should release automation allow a "manual fault lane evidence attached"
  marker when provider credentials cannot run in CI?
