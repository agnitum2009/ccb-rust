# Default Recommended Install

Date: 2026-06-10

## Context

The user wants `ccb_self` to be available by default and strongly recommended
for CCB projects. The role is useful for configuration ownership, runtime
diagnosis, guarded recovery, message-chain repair, and single-agent restart
assistance.

At the same time, automatically rewriting existing project or user configs
would change project topology, tmux layout, provider usage, and resource
consumption without explicit project intent.

## Decision

Install or refresh `agentroles.ccb_self` as a recommended default Role Pack
during install/update Role Pack provisioning.

Bind `ccb_self:codex` to `agentroles.ccb_self` in the built-in blank-project
default config so a project with no user or project config can use
`ccb-config` immediately.

Do not silently bind `agentroles.ccb_self` into existing project or user
`.ccb/ccb.config` files. Adoption remains explicit for any config that
overrides the built-in default:

```bash
ccb roles add agentroles.ccb_self:codex
ccb reload
```

Documentation, CLI examples, and release notes should strongly recommend this
binding for existing CCB projects that want a dedicated maintenance assistant.

## Consequences

- Users get the Role Pack assets by default, so project adoption does not start
  with a separate install step.
- Blank projects get a usable `ccb_self` maintenance route without hand-writing
  config first.
- Existing projects keep their current agent topology until the user chooses to
  add `ccb_self`.
- Update/install failures for `agentroles.ccb_self` are part of Role Pack
  provisioning health, but optional provisioning remains soft unless the user
  forces it with `CCB_INSTALL_ROLES=1`.
