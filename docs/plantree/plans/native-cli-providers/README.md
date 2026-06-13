# Native CLI Providers

Date: 2026-06-13

## Purpose

Add first-class CCB provider support for recently requested native terminal
coding CLIs:

- `kimi`: Moonshot AI Kimi Code CLI, command `kimi`.
- `deepseek`: DeepSeek-oriented Deep Code CLI, command `deepcode`.

The current landing slice makes both providers usable in `.ccb/ccb.config`,
mounts them in managed tmux panes, sends CCB ask prompts, detects replies via
provider-native session/event logs, and exposes diagnostics consistent with
existing pane-backed providers.

## Authority

Product/runtime contracts remain authoritative:

- [../../../ccbd-startup-supervision-contract.md](../../../ccbd-startup-supervision-contract.md)
- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../../managed-provider-completion-reliability-plan.md](../../../managed-provider-completion-reliability-plan.md)

This plan root records the active provider onboarding slice and does not
override the shipped contracts.

## File Map

- [roadmap.md](roadmap.md): current phase, landed work, next tasks, and
  deferred follow-ups.
- [implementation-status.md](implementation-status.md): operational handoff for
  the in-progress implementation.
- [open-questions.md](open-questions.md): unresolved provider behavior or
  rollout questions.
- [topics/source-research.md](topics/source-research.md): upstream CLI source,
  package, install, command, and auth findings.
- [topics/integration-design.md](topics/integration-design.md): CCB provider
  architecture, completion detection, configuration, and testing plan.

## Scope

In scope:

- Provider keys `kimi` and `deepseek`.
- Default executables `kimi` and `deepcode`.
- `KIMI_START_CMD` and `DEEPSEEK_START_CMD` overrides.
- Managed tmux pane startup using the existing simple tmux runtime path.
- Native completion detection using `CCB_REQ_ID` binding plus provider-owned
  Kimi `wire.jsonl` and DeepCode session stores.
- AGY completion alignment to Antigravity transcript logs, so AGY no longer
  relies on `CCB_DONE` as its primary completion signal.
- Empty-reply and timeout diagnostics aligned with existing pane-backed
  providers.
- Unit and isolated source-runtime validation in `/home/bfly/yunwei/test_ccb2`.

Out of scope for the first slice:

- Automatic API key acquisition or account registration.
- Provider-specific key/url shortcut projection in `.ccb/ccb.config`.
- Switching Kimi to a noninteractive `kimi --prompt` execution adapter.
- Supporting multiple DeepSeek community CLIs under one provider key.
