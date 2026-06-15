# Module Map

Date: 2026-05-25

## User-Facing Documentation

- [README.md](../../../README.md): English public README.
- [README_zh.md](../../../README_zh.md): Chinese public README.
- [CHANGELOG.md](../../../CHANGELOG.md): release history source already present
  in the repo.
- [VERSION](../../../VERSION): current source version marker; observed value is
  `7.0.2`.

## Media And Demo Assets

- [assets/](../../../assets): current public images and animations.
- Current README assets observed during inventory:
  - `assets/show.png`
  - `assets/demo.webp`
  - `assets/nvim.png`
  - `assets/readme_previews/video1.gif`
  - `assets/readme_previews/video2.gif`
  - `assets/weixin.jpg`

## Runtime And Config Contracts

- [ccb-config-layout-contract.md](../../ccb-config-layout-contract.md):
  authoritative public config, compact layout, `version = 2` windows topology,
  and sidebar config rules.
- [ccbd-startup-supervision-contract.md](../../ccbd-startup-supervision-contract.md):
  startup, attach, supervision, and kill/shutdown contract.
- [ccbd-diagnostics-contract.md](../../ccbd-diagnostics-contract.md):
  diagnostics and support bundle contract.
- [ccb-agent-sidebar-integration-plan.md](../../ccb-agent-sidebar-integration-plan.md):
  sidebar product/design context and implemented v7 direction.
- [managed-provider-completion-reliability-plan.md](../../managed-provider-completion-reliability-plan.md):
  managed provider completion reliability context relevant to ask/job status
  explanations.

## Current Project Config Example

- The local `.ccb/ccb.config` used during planning had `version = 2` with
  multiple named windows, including a `readme` Codex agent. This is useful as a
  private reference for v7 README examples, but public docs should use
  sanitized examples instead of linking to this project-local runtime file.
