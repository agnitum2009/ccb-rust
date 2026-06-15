# Operation Demo Video And Audio Plan

Date: 2026-05-25

Role: Topic
Status: Active planning
Read when: Designing CCB operation screenshots, demo videos, Bilibili links, or narration
Related: [media-capture-and-asset-plan.md](media-capture-and-asset-plan.md), [tmux-onboarding-runbook.md](tmux-onboarding-runbook.md)

## Purpose

Define how the README should teach CCB operations with images, short clips, and
optionally longer hosted videos. The goal is to reduce tmux/CCB onboarding
friction without making the README heavy.

## Recommendation

Use three media layers:

1. Static screenshots in the README for instant orientation.
2. Short silent WebP/GIF clips in the README for one operation at a time.
3. Full walkthrough videos hosted on Bilibili, linked from the README with
   thumbnail images and short scene descriptions.

Do not start with a standalone audio product. Instead, write a narration script
and subtitles first. If the script proves stable, create voiceover later for the
Bilibili video.

## Why Not Audio First

Audio is useful only when paired with a clear visual sequence. For CCB, the
hardest concepts are visual and operational:

- which pane/window/sidebar row is active;
- where a command is typed;
- how `/ask` moves through Comms;
- what `ccb kill` or `ccb -n` changes;
- how tmux focus, copy/paste, and re-entry work.

Pure audio cannot show these. It also creates extra production work:

- script writing;
- recording or text-to-speech;
- noise cleanup;
- subtitle sync;
- re-recording when commands or UI change.

Recommended path: first create the screen flow and subtitles. Add narration only
after the visual demo is stable.

## Audio Options

| Option | When to use | Pros | Cons |
| :--- | :--- | :--- | :--- |
| No voice, subtitles only | First public version | Fastest, easiest to update, works in README/Bilibili | Less personal, some users prefer narration |
| Human voiceover | Main release video after script stabilizes | Most trustworthy and natural | Harder to revise; needs quiet recording and editing |
| AI voiceover | When frequent iteration is needed | Fast, consistent, easy to regenerate | Can sound generic; may need disclosure depending on style |
| Separate audio/podcast | Not recommended for v7 README launch | Reusable for long-form explanation | Poor fit for visual tmux operations |

Recommendation for v7 README launch:

- README clips: no audio.
- Bilibili overview video: subtitles required; voiceover optional.
- If using voiceover, use the same script as subtitles so updates remain cheap.

## Bilibili Strategy

Use Bilibili for videos longer than 30-60 seconds. Do not commit long video
files to git.

README should include:

- a local thumbnail image under `assets/readme_v7/`;
- a short title such as "5 分钟看懂 CCB v7 多 agent 工作台";
- a Bilibili link;
- a one-line description of what the video covers;
- optionally a transcript or section list in folded details.

Keep README self-contained enough that users can still understand the basics
without opening Bilibili.

## Recommended Video Set

### Video 1: 5-Minute Overview

Purpose: explain why CCB exists and what the v7 workspace looks like.

Scenes:

- single-agent bottleneck in one sentence;
- multi-agent approaches comparison in one slide/table;
- CCB v7 hero workspace with sidebar;
- named windows and agents;
- one `/ask` handoff;
- stop/re-enter/rebuild basics.

Output:

- Bilibili video;
- `assets/readme_v7/video-overview-thumb.png`;
- README link near the hero/quickstart.

### Video 2: 3-Minute tmux Survival Guide

Purpose: help non-tmux users operate CCB without learning general tmux.

Scenes:

- focus a pane/sidebar row;
- switch windows;
- scroll/copy/paste;
- leave and re-enter with `ccb`;
- stop with `ccb kill`;
- force cleanup plus rebuild path.

Output:

- Bilibili video;
- optional short `tmux-basics.webp` in README;
- folded transcript under the tmux guide.

### Video 3: 4-Minute Config And Team Design

Purpose: show compact config, v7 `[windows]`, worktree, and per-agent model/API
overrides.

Scenes:

- compact one-window team;
- v7 named windows;
- sidebar effect;
- worktree agent;
- per-agent `model`, `key`, and `url` example using fake values;
- mention `ccb-config` skill for users who do not want to hand-write config.

Output:

- Bilibili video;
- `windows-topology.png`;
- README config section link.

## Screenshot Set

Screenshots should be more important than videos in the README because they are
visible immediately.

Required screenshots:

- hero workspace with sidebar and named agents;
- annotated sidebar/window/agent/Comms view;
- compact config versus resulting workspace;
- v7 `[windows]` config versus resulting workspace;
- ask flow before/after Comms state;
- tmux basics thumbnail or still frame.

## Short Clip Set

Keep README clips short and silent:

- `quickstart-start.webp`: start and attach.
- `ask-workflow.webp`: ask handoff and Comms update.
- `tmux-basics.webp`: focus/window/copy/paste or a subset.

Each clip should teach one operation and have nearby text explaining what
happened.

## Production Workflow

1. Write a scene script in Markdown.
2. Build a sanitized demo project.
3. Capture static screenshots first.
4. Record short clips from the same scene setup.
5. Edit captions/subtitles.
6. Export optimized README assets.
7. Upload long videos to Bilibili.
8. Add README thumbnail/link/transcript.
9. Verify links, asset paths, file size, and privacy.

## Narration Script Template

Use this before recording any voice:

```text
Scene:
What the user sees:
What command/action happens:
Narration:
Subtitle:
Privacy check:
Retake trigger:
```

## Open Questions

- What Bilibili account/channel should host the videos?
- Should Bilibili videos be Chinese-only first, with English subtitles later?
- Should the first overview video use human voice, AI voice, or subtitles only?
- What terminal theme and font should be used for captures?
- Should thumbnails include Chinese text, English text, or no text?
