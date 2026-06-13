# Reviewer1 Homepage Polish Review

Date: 2026-06-12

## Source

CCB ask job: `job_6092320a91e1`

Artifact:
`.ccb/ccbd/artifacts/text/completion-reply/job_6092320a91e1-art_89d691bcc11a49a2.txt`

SHA256:
`0b7afd9014de511a5110e295710be7113bd56619df36246edf3f08d9095db629`

## Findings

- First-screen order is ready to implement. Reviewer1 strongly endorsed the
  proposed order: hero visual, three values, Quick Start, then rationale.
- Hero asset strategy was blocking. `assets/ccb-promo.png` creates bilingual
  parity, file size, and asset-placement ambiguity.
- Moving the full "Why multi agents" and comparison sections below Quick Start
  is low risk if the top keeps one concise multi-agent positioning sentence and
  the hero shows multiple panes.
- `readme-curator` should wait until after the homepage patch lands.

## Required Plan Changes

- Resolve hero strategy explicitly before editing public README files.
- Generate or optimize language-specific hero assets under `assets/readme_v7/`.
- Reduce the current README badge count from seven to no more than four.
- Add an explicit asset-to-section map so the existing annotated screenshots
  have a clear role if the new promotional hero becomes the top image.

## Follow-Up

The plan adopted the reviewer recommendation in
[../decisions/004-homepage-hero-asset-strategy.md](../decisions/004-homepage-hero-asset-strategy.md).
