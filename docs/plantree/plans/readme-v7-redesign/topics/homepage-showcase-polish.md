# README Homepage Showcase Polish

Date: 2026-06-12

Role: Topic
Status: Active planning
Read when: Optimizing the public GitHub README first screen, hero image,
top navigation, and visible/folded split
Related: [../roadmap.md](../roadmap.md),
[readme-implementation-blueprint.md](readme-implementation-blueprint.md),
[readme-rewrite-execution-plan.md](readme-rewrite-execution-plan.md),
[media-capture-and-asset-plan.md](media-capture-and-asset-plan.md),
[../decisions/005-readme-design-non-drift-contract.md](../decisions/005-readme-design-non-drift-contract.md)

## Trigger

Maintainer feedback: the current CCB README homepage has too much text, looks
too plain, and does not focus the first screen on the product. The goal of this
pass is to improve the public GitHub landing experience before doing another
README implementation patch.

## Current First-Screen Audit

Observed in `README.md` and `README_zh.md` on 2026-06-12:

- The top area has a clear title and badges, but the first major sections are
  still rationale-heavy.
- "Why multi agents" and approach comparison appear before the product visual,
  so the reader sees argumentation before seeing CCB.
- The screenshot exists, but it is not the first strong visual signal.
- Navigation contains many links, which makes the top feel busy rather than
  guided.
- The first-read path is closer to a design memo than a GitHub product
  homepage.

Useful existing assets:

- `assets/ccb-promo.png`: newly generated annotated CCB promotional screenshot.
- `assets/readme_v7/ccb-test2-terminal-annotated.png`: existing Chinese v7
  annotated screenshot.
- `assets/readme_v7/ccb-test2-terminal-annotated-en.png`: existing English v7
  annotated screenshot.

Reviewer feedback on 2026-06-12 made the hero asset strategy a blocking
pre-implementation issue. See
[../history/reviewer1-homepage-polish-2026-06-12.md](../history/reviewer1-homepage-polish-2026-06-12.md)
and
[../decisions/004-homepage-hero-asset-strategy.md](../decisions/004-homepage-hero-asset-strategy.md).
The stable anti-drift authority for future README homepage edits is
[../decisions/005-readme-design-non-drift-contract.md](../decisions/005-readme-design-non-drift-contract.md).

## External README Patterns To Borrow

Sources surveyed in the maintainer discussion pass:

| Project | Pattern To Borrow | CCB Translation |
| :--- | :--- | :--- |
| Dify | Big visual first, then concise product definition and deployment entry points. | Put a strong CCB terminal visual directly under the header before the rationale. |
| n8n | Banner, one-sentence product claim, screenshot, key capabilities, quick start. | Use one sentence plus three capability bullets before any comparison table. |
| uv | Evidence appears early: concise positioning, install, highlights, benchmark visual. | Treat the CCB tmux screenshot as proof of the product, not a later illustration. |
| Cline | Several product surfaces are exposed as simple entry points. | Present "run", "delegate", "review", and "recover" as short user paths. |
| Zellij | Terminal product uses demo/screenshot first, then installation and usage. | CCB should act like a terminal workspace README, not a long architecture note. |
| Awesome README / README guides | Good README pages use screenshots, GIFs, clear formatting, and a short elevator pitch. | Keep the first screen as pitch plus image plus first action. |

## Design Goal

Make the GitHub first screen answer four questions quickly:

1. What is CCB?
2. What does it look like?
3. Why is it different from one agent or invisible orchestration?
4. How do I try it?

Non-goals for this pass:

- Do not redesign CCB runtime behavior.
- Do not add new command semantics.
- Do not rewrite the full manual-style lower README yet.
- Do not create a separate documentation site.
- Do not add a large marketing narrative that obscures concrete CLI usage.

## Recommended First-Screen Shape

Target order for both `README_zh.md` and `README.md`:

1. Centered title: `CCB`
2. One-line positioning.
3. Three small badges at most: version, platform, providers.
4. Language switch and 4-5 short navigation links.
5. Hero image: use canonical language-specific images under
   `assets/readme_v7/`, generated or optimized from the newer promo-style CCB
   composition. Do not directly reference `assets/ccb-promo.png` from public
   READMEs.
6. Three compact value bullets or a three-column table.
7. Compact supported-CLI logo/badge strip showing Codex, Claude, Gemini, Kimi,
   OpenCode, Antigravity, and Droid at a glance.
8. User and developer documentation links should remain visible near the top:
   `docs/manuals/user-guide/` and `docs/manuals/developer-guide/`.
9. Quick Start within the first visible section after the image.
10. Move "Why multi agents" and "Which approach" below Quick Start, with detail
   folded.

Suggested English positioning:

```md
CCB is a visible multi-agent CLI workspace for running Codex, Claude, Gemini,
Kimi, OpenCode, and other real provider CLIs side by side in one project-owned
tmux session.
```

Suggested Chinese positioning:

```md
CCB 是一个可见、可控的多 Agent CLI 工作台，用一个项目级 tmux 会话同时管理
Codex、Claude、Gemini、Kimi、OpenCode 等真实 CLI。
```

Suggested visible value bullets:

| Value | Short Copy |
| :--- | :--- |
| See the work | Every agent is a real terminal pane, not a hidden background worker. |
| Mix providers | Use Codex, Claude, Gemini, Kimi, OpenCode, Antigravity, and per-agent models or keys together. |
| Keep control | Start, attach, delegate, review, rebuild, and stop the project workspace explicitly. |

## Proposed Chinese Top Outline

```md
# CCB

可见、可控的多 Agent CLI 工作台。

[快速开始](#快速开始) · [界面速览](#界面速览) · [配置团队](#配置-agent-团队) · [English](README.md)

<img src="assets/readme_v7/ccb-hero-zh.png" alt="CCB 多 Agent CLI 工作台" width="960">

## 为什么用 CCB？

| 看得见 | 混合 provider | 项目级控制 |
| :--- | :--- | :--- |
| 每个 agent 都是真实 CLI pane。 | 同时运行 Codex、Claude、Gemini、Kimi、OpenCode 等。 | 启动、委派、审查、恢复、停止都在一个项目里完成。 |

## 快速开始
...

## 界面速览
...

## 为什么需要多 agents
...
```

## Visual Rules

- Keep one dominant screenshot in the first screen; do not stack multiple
  screenshots before Quick Start.
- Use a real or promotional terminal image that shows CCB as the product.
- Keep annotations sparse and descriptive. Avoid long callout paragraphs on the
  image itself.
- Keep badges visually quiet; do not let shields dominate the first screen.
  Cut the current seven-badge header to no more than four visible badges, with
  three preferred: version, platform, and providers.
- Use tables only where they reduce text length. Avoid large comparison tables
  before the first command.
- Put conceptual rationale after the user sees the product.
- Keep the top navigation under one line on typical desktop widths.

## Asset-To-Section Map

Use these assets in the next README polish patch:

| Asset | Language | README Section | Status |
| :--- | :--- | :--- | :--- |
| `assets/readme_v7/ccb-hero-zh.png` | Chinese | First-screen hero in `README_zh.md` | Generated from the newer promo-style CCB composition and optimized for README load |
| `assets/readme_v7/ccb-hero-en.png` | English | First-screen hero in `README.md` | English version generated from the same promo-style CCB composition and optimized for README load |
| `assets/readme_v7/ccb-test2-terminal-annotated.png` | Chinese | UI Tour detail image if the hero uses a cleaner promotional crop | Existing fallback/detail asset |
| `assets/readme_v7/ccb-test2-terminal-annotated-en.png` | English | UI Tour detail image if the hero uses a cleaner promotional crop | Existing fallback/detail asset |
| `assets/ccb-promo.png` | Chinese/source reference | Social/share/reference asset and promo composition source, not the direct README hero path | Existing; keep out of public README references |

Hero asset requirements:

- Generate or optimize the canonical hero images under `assets/readme_v7/`.
- Keep each canonical hero reasonably small for GitHub page load; target
  `<300KB` when image quality remains acceptable.
- Keep Chinese and English embedded annotations language-specific, or keep the
  hero annotation-free and move all labels into README text.
- Preserve the `ccb_self` callout in both hero languages when using the
  promo-style composition.
- Avoid duplicating the same visual twice before Quick Start. If the hero is
  annotated enough, the UI Tour can use a table directly below it instead of a
  second image.

## Information Architecture Change

Recommended top-level order for the polish patch:

1. Hero and product proof.
2. Quick Start.
3. Interface tour.
4. Why multi agents.
5. Approach comparison.
6. Daily operation.
7. tmux basics.
8. Configure agents.
9. Collaboration.
10. Install/update details.
11. FAQ, credits, release notes.

The previous v7 rewrite put rationale before visual proof. This pass should
reverse that ordering for public GitHub conversion while preserving the deeper
comparison content below the first action path.

## README Maintenance Skill Idea

Candidate skill name: `readme-curator`.

Purpose:

- Maintain README first-screen quality and bilingual parity.
- Keep CCB's homepage from drifting back into a long design memo.
- Check that screenshots, links, badges, section order, and fold policy stay
  aligned with the plan.

Suggested skill checklist:

- First screen includes positioning, visual proof, three core values, and Quick
  Start entry.
- First screen does not include long rationale, deep comparison, release
  history, or advanced config.
- `README.md` and `README_zh.md` have matching section order and asset usage.
- Image paths and local links resolve.
- Badge count stays small.
- Public command examples avoid secrets and source-only test commands.
- Advanced material is folded or moved to docs.

Create this skill only after the homepage polish patch is accepted; otherwise
the skill may encode a design that is still under review.

Reviewer1 agreed with deferring this skill until after the homepage patch lands.

## Acceptance Criteria

- GitHub first screen shows CCB itself before long rationale.
- A new reader can state what CCB is after one screen.
- Quick Start is reachable immediately after the hero image.
- The top uses one dominant image and no more than three visible value points.
- The first screen feels like a product homepage, while deeper sections still
  preserve the existing rigorous comparison and configuration guidance.
- Chinese and English README files remain structurally parallel.
- Supported CLIs are visible near the first screen as a compact logo/badge
  strip, without expanding the header badge count.
- Reviewer feedback from `reviewer1` is recorded or converted into follow-up
  edits before implementation.
- The README header badge count is reduced to no more than four total badges.
- Hero asset references are canonical `assets/readme_v7/` paths, not
  `assets/ccb-promo.png`.
- Top navigation keeps user and developer documentation links discoverable.
- The first-read path emphasizes that `ccb_self` is CCB's built-in
  self-understanding expert for CCB usage, config design, diagnostics, recovery,
  and workflow repair.
- Any future homepage edit that changes hero order, install path, documentation
  links, or `ccb_self` positioning updates the non-drift decision first.

## Reviewer1 Discussion Request

Ask `reviewer1` to review this topic before implementation. Requested focus:

- Whether the new first-screen order is better for GitHub readers.
- Whether `assets/ccb-promo.png` should become the immediate hero image or a
  canonical `assets/readme_v7/` screenshot should be regenerated.
- Whether moving "Why multi agents" below Quick Start risks weakening CCB's
  positioning.
- Whether a `readme-curator` skill is useful now or should wait until after the
  README patch lands.

## Reviewer1 Outcome

Reviewer1 completed review on 2026-06-12:

- Endorsed the proposed first-screen order as a clear improvement.
- Marked hero asset strategy as blocking before implementation.
- Recommended language-specific hero assets under `assets/readme_v7/`.
- Considered moving "Why multi agents" below Quick Start low risk as long as
  one concise positioning sentence remains visible.
- Agreed that `readme-curator` should wait until after the homepage patch is
  accepted.
