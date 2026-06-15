# Dedicated API Lanes For Status Testing

Date: 2026-05-27

## Context

Provider status accuracy cannot be proven with only mocks or the developer's
normal API credentials. Fault cases such as disconnects, invalid auth,
unavailable models, rate limits, and partial stream failures need repeatable
test routes.

## Decision

Sidebar provider-activity validation uses dedicated test agents and API lanes:

- stable Codex
- fault Codex
- stable Claude
- fault Claude
- invalid-auth lane
- unavailable-model lane

Fault lanes should route through a controllable local proxy or mock endpoint
where possible, and they should be configured per agent through `.ccb/ccb.config`
API shortcut/profile authority rather than global shell environment.

## Consequences

- Normal developer/provider credentials are not mutated by status tests.
- Fault drills can be repeated without corrupting stable baseline panes.
- Release validation can include real failure evidence, not only unit tests.
- Secrets must stay outside committed plan/config files.
