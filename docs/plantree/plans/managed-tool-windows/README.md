# Managed Tool Windows Plan

Date: 2026-05-30

## Purpose

Plan first-class CCB-managed non-agent windows such as a default `neovim`
window. A tool window is visible in the sidebar as a single window row and is
managed by the project tmux namespace, but it is not an agent and must not
participate in ask routing, provider runtime, health monitoring, completion
tracking, or Comms.

This plan exists separately from the agent hot-reload plan because the core
model is a new topology primitive. Reload is one integration path, not the
authority for the feature.

## File Map

- [roadmap.md](roadmap.md): current implementation sequence and gates.
- [open-questions.md](open-questions.md): unresolved questions only.
- [topics/config-and-topology-contract.md](topics/config-and-topology-contract.md):
  proposed `tool_windows` config shape, validation, identity, and user-facing
  semantics.
- [topics/namespace-sidebar-reload-design.md](topics/namespace-sidebar-reload-design.md):
  namespace materialization, sidebar/project-view payloads, and explicit reload
  behavior for tool windows.
- [topics/neovim-lazyvim-provisioning.md](topics/neovim-lazyvim-provisioning.md):
  install/update provisioning for a CCB-managed Neovim and LazyVim profile,
  including tmux compatibility.
- [topics/neovim-system-optimization.md](topics/neovim-system-optimization.md):
  second-phase plan for making the managed Neovim profile useful and
  diagnosable across Linux, macOS, and WSL, including folders, Markdown,
  images, clipboard, and opener behavior.
- [history/neovim-local-plugin-lab-2026-06-13.md](history/neovim-local-plugin-lab-2026-06-13.md):
  isolated local Linux/tmux plugin lab for folder, Markdown, image, parser,
  browser-preview, opener, and clipboard capability checks.
- [topics/test-matrix.md](topics/test-matrix.md): automatic and manual tests,
  including `test_ccb2` validation.
- [decisions/001-tool-windows-are-not-agents.md](decisions/001-tool-windows-are-not-agents.md):
  decision record for keeping tool windows out of agent/provider runtime.
- [decisions/002-isolated-managed-neovim-profile.md](decisions/002-isolated-managed-neovim-profile.md):
  decision record for installing Neovim/LazyVim into CCB-owned isolated paths.
- [decisions/003-neovim-enhancement-defaults.md](decisions/003-neovim-enhancement-defaults.md):
  decision record for capability-gated Neovim folder, Markdown, image,
  browser-preview, clipboard, and plugin-pinning defaults.

## Related Sources

- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../../ccbd-startup-supervision-contract.md](../../../ccbd-startup-supervision-contract.md)
- [../ccbd-agent-hot-reload/README.md](../ccbd-agent-hot-reload/README.md)
- [../ccbd-agent-hot-reload/topics/non-disruptive-hot-load-design.md](../ccbd-agent-hot-reload/topics/non-disruptive-hot-load-design.md)
- [../sidebar-provider-activity/README.md](../sidebar-provider-activity/README.md)
- [../../baseline/runtime-flows.md](../../baseline/runtime-flows.md)
- [../../baseline/storage-and-state.md](../../baseline/storage-and-state.md)

## Scope

In scope:

- A config-level `tool_windows` concept for windows that run a command such as
  `nvim`.
- Cold-start materialization of managed tool windows.
- Project view and sidebar rendering as one window row with no child agent row.
- Explicit `ccb reload` add/remove behavior for idle managed tool windows.
- Project/session-scoped tmux identity and UI settings.
- `ccb update` / `install.sh install` interactive provisioning for
  CCB-managed Neovim and LazyVim, with non-interactive skip,
  `CCB_INSTALL_NEOVIM=0` opt-out, and `CCB_INSTALL_NEOVIM=1` fail-hard mode.
- tmux compatibility settings applied at CCB session/window scope, not through
  user-global tmux config edits.
- Tests proving tool windows do not become agents or provider runtime records.

Out of scope for the first slice:

- Tool windows participating in `ask`, Comms, provider status, or completion.
- Arbitrary tool pane layouts inside one tool window.
- Background config watching.
- Automatic replacement of a running tool command after its command changes.
- Treating a tool window as a terminal multiplexer workspace independent of
  CCB ownership.
- Mutating a user's existing `~/.config/nvim`, Neovim data/cache/state
  directories, or global tmux configuration.
