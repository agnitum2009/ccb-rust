# Managed Tool Windows Roadmap

Date: 2026-05-30

## Done

- Confirmed current explicit `[windows]` topology is agent-only: every window
  value must contain provider-declared agent leaves and `WindowSpec` requires at
  least one `agent_name`.
- Confirmed the current sidebar already renders a window row independently of
  child agent rows, so a window payload with `agents: []` can support the
  desired single-row visual model once project view emits the right window
  shape.
- Confirmed the current namespace materializer creates managed windows,
  sidebars, and agent panes from `ProjectConfig.windows`; tool windows need a
  separate materialization path so they do not create provider runtime
  authority.
- Recorded the decision that managed tool windows are not agents and must not
  be represented as fake providers.
- Recorded the decision that CCB-managed Neovim/LazyVim must be isolated from
  the user's existing Neovim home and global tmux config.
- Defined the first-class `tool_windows` config/topology model in
  [topics/config-and-topology-contract.md](topics/config-and-topology-contract.md).
- Implemented explicit project-view/sidebar shape for tool windows: one window
  row, no child agent row.
- Implemented cold-start and explicit reload add/remove namespace behavior for
  managed tool windows without provider runtime authority.
- Added `ccb tools doctor/install/update neovim` provisioning with isolated
  `ccb-nvim` wrapper/profile, official Neovim tarball fallback, checksum
  verification, and install/update soft mode.
- Added first-slice CCB tmux compatibility policy for managed tool windows:
  focus events and low escape-time are applied through CCB-owned tmux backend
  policy rather than user-global tmux config.
- Completed the automatic matrix for config loading, project view/sidebar,
  namespace materialization, reload dry-run/apply, Neovim provisioning, Rust
  sidebar parsing/rendering, and plan-tree link consistency.
- Completed an isolated Linux/tmux Neovim plugin lab for folder opening,
  Markdown rendering, image handling, browser preview, parser runtime paths,
  opener, and clipboard capability checks. Results are recorded in
  [history/neovim-local-plugin-lab-2026-06-13.md](history/neovim-local-plugin-lab-2026-06-13.md).
- Recorded managed Neovim enhancement defaults in
  [decisions/003-neovim-enhancement-defaults.md](decisions/003-neovim-enhancement-defaults.md).

## In Progress

- Run the live `/home/bfly/yunwei/test_ccb2` Neovim add/remove reload scenario
  from [topics/test-matrix.md](topics/test-matrix.md).

## Next

1. Install the current source build into the test environment and complete live
   validation that existing agents survive tool-window add/remove
   and `ccb ask` still routes through the daemon.
2. Record any live tmux/Neovim compatibility issues in the test matrix or manual
   issue log.
3. Implement the managed Neovim system-optimization foundation from
   [topics/neovim-system-optimization.md](topics/neovim-system-optimization.md):
   preserve parser runtime paths, add read-only capability diagnostics, and
   keep doctor from mutating parser/plugin state.
4. Prototype the first enhanced profile overlay in this order: Snacks folder
   baseline with watcher disabled, in-buffer Markdown rendering gated on parser
   readiness, image external-open fallback, then clipboard/opener keymaps.
5. Add macOS, WSL home, and WSL mounted-drive manual checks before enabling
   rich media defaults beyond Linux/tmux.

## Deferred

- Multiple panes inside a tool window.
- Tool command replacement or restart policy.
- Background config watching.
- Tool-specific sidebar activity/status beyond focus/liveness.
- Cross-project/global tool definitions.
- Bundling every Neovim release binary directly inside the CCB release tarballs
  instead of provisioning from a versioned manifest.
