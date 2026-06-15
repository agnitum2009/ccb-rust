# Media Capture And Asset Plan

Date: 2026-05-26

Role: Topic
Status: Active planning
Read when: Creating screenshots, GIF/WebP demos, or README asset references
Related: [roadmap.md](../roadmap.md), [operation-demo-video-and-audio-plan.md](operation-demo-video-and-audio-plan.md)

## Current Asset Inventory

Existing public media:

- `assets/show.png`
- `assets/demo.webp`
- `assets/nvim.png`
- `assets/readme_previews/video1.gif`
- `assets/readme_previews/video2.gif`
- `assets/weixin.jpg`

These assets should be preserved until replacement references are merged and
reviewed. Do not delete old assets as part of planning.

Draft v7 assets captured from `/home/bfly/yunwei/ccb_test2`:

- `assets/readme_v7/ccb-test2-workspace-annotated.png`: annotated full workspace
  with sidebar, Comms, two Codex panes, one Claude pane, and active-pane outline.

These are planning references, not final public README hero assets. The current
full-workspace image was rendered from tmux capture text. Maintainer decision:
regenerate real terminal raster screenshots for the public README, then explain
each important visible area in nearby text.

Regenerated real dark terminal assets now used by the README:

- `assets/readme_v7/ccb-test2-terminal.png`: cropped real terminal capture.
- `assets/readme_v7/ccb-test2-terminal-annotated.png`: Chinese annotated hero.
- `assets/readme_v7/ccb-test2-terminal-annotated-en.png`: English annotated hero.

Maintainer cleanup decision: do not keep separate local/detail screenshots for
sidebar, Codex panes, or Claude panes in the first README pass. The main
annotated terminal screenshot plus nearby explanation table is enough.

## Target Folder

Use a dedicated folder for the redesign:

```text
assets/readme_v7/
```

Candidate committed artifacts:

- `ccb-test2-terminal.png`: real terminal first-screen v7 project view with
  sidebar.
- `ccb-test2-terminal-annotated.png` and
  `ccb-test2-terminal-annotated-en.png`: annotated variants explaining the major
  areas.
- `ccb-test2-workspace-annotated.png`: planning reference only unless replaced
  by a real terminal equivalent.
- `windows-topology.png`: named tmux windows with sidebar visible.
- `quickstart-start.webp`: short start/attach flow.
- `ask-workflow.webp`: `/ask` or `$ask` delegation and Comms update.
- `tmux-basics.webp`: mouse focus, window switch, scroll/copy/paste, detach or
  recover.
- `editor-integration.png`: editor plus CCB project workspace.

Maintainer decision: commit optimized public README assets in this folder and do
not commit raw recordings for this pass. Regenerated screenshots should use the
existing dark terminal visual style, a wide README-friendly frame, and sparse
numbered annotations. Detailed explanations belong in README text tables rather
than inside the image.

## Capture Scenarios

| Scene | Purpose | Required Evidence |
| :--- | :--- | :--- |
| Hero sidebar | Show what v7 looks like before text details | Sidebar, named windows or a clear managed window, at least two live agents, no secrets |
| First run | Deferred until later demo/video design | Clean demo project, visible `ccb` startup, final attached UI |
| Windows topology | Teach `[windows]` value | Config snippet plus resulting windows/sidebar |
| Ask delegation | Show agent collaboration | User asks one agent, target accepts/runs/completes, Comms reflects state |
| tmux survival | Teach non-tmux operations | Focus/switch/scroll/copy/paste or safe fallback wording |
| Editor integration | Show realistic coding use | Editor and CCB visible without private repo content |

The capture source should be a sanitized demo project, not the maintainer's real
worktree.

## Privacy And Quality Rules

- Use a sanitized demo project, not the maintainer's real worktree.
- Hide API keys, account identifiers, local secrets, private prompts, and
  irrelevant absolute paths.
- Use a consistent dark terminal theme and wide terminal size across captures.
- Prefer short animations over long recordings; each animation should teach one
  operation.
- Pair every animation with nearby text that explains the result.
- Optimize file size before committing.
- Include useful alt text in README references.

## Capture Pipeline Options

Preferred options to confirm during implementation:

- Static real terminal screenshots from the terminal/editor after a real v7
  start.
- Short terminal recordings converted to WebP/GIF with `ffmpeg` or an equivalent
  local tool.
- Longer narrated walkthroughs hosted externally, likely on Bilibili, with a
  README thumbnail and link instead of committing large video files.
- `asciinema` only if the rendered output can represent the sidebar and tmux
  layout clearly enough for GitHub readers.

Raw recordings should stay outside git unless a later decision creates a media
source archive.

No README video recording or concrete task demo is needed for the current pass.
Interface screenshots and region explanation come first; Bilibili-hosted
walkthrough videos and task-demo scenarios can be created after the README copy
is stable.

## Verification Checklist

- Asset path exists.
- Asset displays correctly in GitHub Markdown.
- File size is acceptable for README load time.
- Alt text describes the operation, not just "screenshot".
- Screenshot is current v7 behavior or labeled as illustrative.
- No secret or private content is visible.
- Final hero screenshot is a real terminal raster screenshot, not only a
  text-rendered mock/capture.
- Image annotations are sparse and numbered; detailed bilingual explanation
  stays in README tables.
