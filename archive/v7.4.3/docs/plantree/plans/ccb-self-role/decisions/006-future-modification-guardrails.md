# Future Modification Guardrails

Date: 2026-06-11

## Context

`ccb_self` is now part of the built-in blank-project default so new projects
can route CCB configuration work to the private `ccb-config` skill without
hand-editing a project config first.

Two details are easy to regress during future maintenance:

- the canonical Role Pack id is `agentroles.ccb_self`, while the earlier
  singular `agentrole.ccb_self` spelling is legacy compatibility only;
- CCB maintenance heartbeat must remain disabled by default and manually
  enabled by config.

These choices affect defaults, install/update provisioning, config rendering,
docs, role lookup, and startup behavior.

## Decision

Future `ccb_self` changes must preserve these invariants:

1. The canonical Role Pack id is `agentroles.ccb_self`.
   - New generated config, docs, CLI examples, provisioning lists, tests, and
     role locks should use `agentroles.ccb_self`.
   - `agentrole.ccb_self` may exist only as legacy input compatibility,
     migration evidence, or historical draft-path text.
   - Runtime normalization should canonicalize legacy `agentrole.ccb_self`
     inputs to `agentroles.ccb_self`.
2. Blank-project built-in config may include `ccb_self:codex` bound to
   `agentroles.ccb_self` so `ccb-config` is available immediately.
   - Existing project/user configs must not be silently rewritten.
   - Projects that override the built-in default still add `ccb_self`
     explicitly when they want the maintenance assistant.
3. Maintenance heartbeat is opt-in.
   - Default config must keep `[maintenance.heartbeat].enabled = false`.
   - Installing, refreshing, or default-mounting `ccb_self` must not enable
     heartbeat and must not start a heartbeat runner.
   - Features that use maintenance heartbeat must require explicit config such
     as:

```toml
[maintenance.heartbeat]
enabled = true
```

4. Future changes touching these surfaces must update the relevant contracts,
   docs, and tests together:
   - `docs/ccb-config-layout-contract.md`
   - `docs/ccbd-startup-supervision-contract.md`
   - README / README_zh user guidance
   - default config rendering/loading tests
   - install/update Role Pack provisioning tests
   - maintenance heartbeat startup tests

## Consequences

- Future `ccb_self` work has one durable naming authority for the Role Pack id.
- Blank projects remain easy to configure through `ccb_self`, but background
  semantic maintenance stays user-controlled.
- Heartbeat-based self-supervision can still be added later, but only as an
  explicit opt-in feature.
- Compatibility with older `agentrole.ccb_self` text remains a migration
  concern, not the source of truth for new behavior.
