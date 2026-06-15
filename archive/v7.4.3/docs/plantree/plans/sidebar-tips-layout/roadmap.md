# Sidebar Tips Layout Roadmap

Date: 2026-05-27

## Done

- Confirmed the current Rust TUI uses a fixed two-panel `50% / 50%` vertical
  split for tree and Comms.
- Confirmed the current Comms renderer truncates by available panel height,
  not by a fixed item count.
- Confirmed `project_view` already exposes `namespace.sidebar`, but the Rust
  sidebar model does not yet parse a full sidebar view configuration.
- Confirmed `ui.sidebar.mode`, `width`, and `bottom_height` are topology-facing
  settings today.
- Implemented static three-panel sidebar rendering in the Rust TUI.
- Implemented compact one-line Comms rows with a default visible limit of 5.
- Added default tmux Tips rendering in the bottom panel.
- Added UI-only `[ui.sidebar.view]` parsing and `project_view` delivery for
  `agents_height`, `comms_limit`, `comms_compact`, `tips_enabled`, and `tips`.
- Kept sidebar view config out of topology/config identity so Tips text changes
  can hot-reload without namespace recreation.

## In Progress

- Dogfood the three-panel sidebar in a real tmux project pane and tune exact
  height defaults if the first 33%/5-row split feels cramped.

## Next

- Run a live tmux smoke with a project-level `.ccb/ccb.config` override for
  `[ui.sidebar.view]`.
- Decide whether `comms_limit = 5` should remain user-facing in the first
  release note or stay documented as an advanced option.

## Deferred

- User-editable Tips details/details popup.
- Per-window Tips overrides.
- Scrollable sidebar panels.
- True font-size control inside the sidebar pane. Terminal TUI rendering cannot
  own this reliably.
- Persisted per-sidebar-pane UI state beyond the existing local selection
  model.

## Phase Gates

Phase 1 is complete when:

- The sidebar renders three panels in desktop-sized tmux panes.
- Comms shows at most 5 visible rows by default.
- Tips shows useful default tmux hints.
- Existing window/agent mouse focus and Comms action clicks still work.

Phase 2 is complete when:

- `.ccb/ccb.config` can override Tips text and compact display settings without
  forcing namespace topology recreation.
- The Rust sidebar picks up updated settings through the normal `project_view`
  refresh path.
- Config validation rejects unsupported sidebar view fields with clear errors.
