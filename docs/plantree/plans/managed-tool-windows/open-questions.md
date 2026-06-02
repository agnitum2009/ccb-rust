# Managed Tool Windows Open Questions

Date: 2026-05-30

## Questions

- Should the first command contract be a single shell command string only, or
  should it also support an argv array before the feature ships?
- When a tool command exits, should CCB leave the pane open, restart it, or show
  an exited marker until explicit reload/restart?
- Should command changes be blocked until a future explicit `tool restart`
  policy exists, or should they be treated as remove-and-add when the old tool
  pane is idle/exited?
- Which clipboard lane should be the standard fallback inside tmux when the OS
  clipboard helper is missing: Neovim OSC52, tmux `set-clipboard`, or explicit
  platform helper installation?
