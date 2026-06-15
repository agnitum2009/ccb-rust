# Three-Panel Sidebar

Date: 2026-05-27

## Goal

Use the sidebar height more deliberately:

```text
Agents / windows  about 1/3
Comms             compact, at most 5 rows
Tips              remaining bottom panel
```

The intent is not to make the sidebar denser for its own sake. The intent is to
keep the project map, recent ask/job communication state, and tmux operating
hints visible at the same time.

## Current Implementation Facts

- `tools/ccb-agent-sidebar/src/tui.rs` currently splits the sidebar into two
  equal vertical panels.
- Comms rows currently consume one to three terminal lines depending on preview
  and reason text.
- Comms visible count is indirectly controlled by panel height.
- Mouse hit testing is tied to the current two-area layout.
- `project_view` already includes `namespace.sidebar`, but the Rust model only
  parses the subset it currently renders.
- Current `ui.sidebar.mode`, `width`, and `bottom_height` participate in
  topology/config identity decisions.

## Proposed Layout

Use a helper that returns three areas instead of two:

```text
sidebar_areas(area) -> SidebarAreas {
  tree,
  comms,
  tips,
}
```

Recommended sizing:

- `tree`: roughly 33-35% of height, with a minimum enough for the window/agent
  tree header and a few rows.
- `comms`: enough for border plus up to 5 compact rows.
- `tips`: remaining height.

Small panes should degrade in this order:

1. Keep tree visible.
2. Keep at least one Comms row if possible.
3. Hide or compress Tips first.

## Comms Rendering

Default Comms should become a compact single-line row:

```text
↻ X ⌫ a2>a3 run fix ask...
```

Rules:

- limit visible rows to 5 by default;
- preserve action colors and status colors;
- truncate preview to available width;
- do not show full job ids;
- show reason text only when it can fit into the same compact row or in a
  future selected-detail mode.

The current multi-line shape can remain useful later for a detail popup or
selected-row expansion.

## Tips Rendering

Default Tips should use short tmux key hints, not full prose:

```text
C-b d  detach
C-b h/j/k/l pane
C-b H/J/K/L resize
C-b o  next pane
C-b z  zoom
C-b w  tree
C-b n/p next/prev
C-b 0-9 jump win
C-b [  copy mode
copy: PgUp/PgDn
copy: v select
copy: y yank
copy: q exit
C-b ]  paste
C-b c  new win
C-b ,  rename
C-b ?  keys
```

Keep Tips independent from CCB state. It is guidance text, not a status panel.

## Config Shape

Preferred eventual shape:

```toml
[ui.sidebar.view]
agents_height = "33%"
comms_limit = 5
comms_compact = true
tips_enabled = true
tips = [
  "C-b d  detach",
  "C-b h/j/k/l pane",
  "C-b H/J/K/L resize",
  "C-b o  next pane",
  "C-b z  zoom",
  "C-b w  tree",
  "C-b n/p next/prev",
  "C-b 0-9 jump win",
  "C-b [  copy mode",
  "copy: PgUp/PgDn",
  "copy: v select",
  "copy: y yank",
  "copy: q exit",
  "C-b ]  paste",
  "C-b c  new win",
  "C-b ,  rename",
  "C-b ?  keys",
]
```

This view config should be treated as UI-only. It should not redefine managed
windows, agents, sidebar projection, pane geometry, or topology signatures.

## Hot Reload Path

The preferred hot-reload path is:

1. config loader parses UI-only sidebar view settings;
2. `project_view` includes a normalized sidebar view payload;
3. Rust model deserializes the payload;
4. sidebar refresh loop picks up changes through the normal `project_view`
   TTL.

The Rust sidebar should not independently read `.ccb/ccb.config`. `ccbd`
remains the source of normalized project/UI state.

## Risks

- Putting Tips fields into the existing topology-facing `SidebarSpec` could
  cause harmless text changes to trigger daemon/config drift behavior.
- Tight layout can break mouse hit testing if the two-panel assumptions remain.
- Real font-size reduction is not a Rust TUI feature; readability must come
  from shorter text and stable truncation.
- A 5-row Comms limit may hide useful recent failures if the feed ordering is
  not tuned.

## Test Notes

Minimum automated coverage:

- Rust render test for three panel borders and default Tips text.
- Rust render test that Comms renders at most 5 compact rows.
- Rust mouse-coordinate test for tree focus after layout split.
- Rust mouse-coordinate test for Comms retry/cancel/clear after layout split.
- Python config-loader tests for future UI-only sidebar view fields.
- ProjectView test proving view-only settings are exposed without changing
  topology fields.
