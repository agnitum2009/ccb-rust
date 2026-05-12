---
name: ccb-github
description: Maintain this CCB project's GitHub-facing release surface. Use when preparing, publishing, auditing, or fixing CCB releases; updating README.md, README_zh.md, CHANGELOG.md, VERSION, GitHub release notes/assets, repository description/topics, or GitHub Actions release/test status.
---

# CCB GitHub Release Maintainer

## Core Rule

Treat GitHub as the user-facing product page. A release is not done until local version files, both READMEs, changelog, GitHub Release, release assets, and Actions status all agree.

Use repository `SeemSeam/claude_codex_bridge` unless the user explicitly gives a different repo.

## Quick Audit

From the CCB repo root, run the bundled checker before and after release work:

```bash
CHECKER="dev_tools/skills/ccb-github/scripts/check_release_state.py"

python "$CHECKER" --phase prepare --repo SeemSeam/claude_codex_bridge
python "$CHECKER" --phase published --repo SeemSeam/claude_codex_bridge
```

The checker is read-only. It catches mechanical drift, but still manually inspect the top of `README.md` and `README_zh.md` because stale "What's New" prose can be semantically wrong even when version numbers are correct.

## Decision Tree

- Before tagging: run `--phase prepare`; fix every FAIL before creating a tag.
- After pushing a tag or creating a release: run `--phase published`; fix every FAIL before reporting success.
- After an interruption: run both phases, then follow the recovery runbook below from the first failing state.
- During README-only maintenance: still run `--phase prepare` so version badges, release notes, install URLs, and memory wording stay aligned.

## Release Preparation Checklist

Update these files together:

- `VERSION`
- `ccb` `VERSION = "..."`
- `CHANGELOG.md`
- `README.md`
- `README_zh.md`

README requirements:

- Top version badge must match the new version.
- "What's New" / "最新亮点" must describe the current release, not an older milestone; compare it against the newest `CHANGELOG.md` section and ensure it covers the most important user-facing bullets.
- "Config Control" / "配置控制" must stay aligned with current `.ccb/ccb.config` behavior.
- Keep the shared memory wording concise: `.ccb/ccb_memory.md` is the project-wide shared memory document.
- Do not reintroduce root `CCB.md` support or mention it as a current feature.
- Install commands must point at the actual public GitHub repo.
- Release Notes / 新版本记录 must include the new version near the top.

GitHub repo homepage requirements:

- `gh repo view SeemSeam/claude_codex_bridge --json description,homepageUrl,repositoryTopics,latestRelease`
- Description and topics should match the current public positioning.
- If README install URLs or badge links point to an old owner, fix them before tagging.

## Local Verification

Run at least:

```bash
pytest -q
python -m compileall -q lib ccb
git diff --check
python scripts/build_linux_release.py --allow-dirty --output-dir dist-release-local
```

For startup, tmux, ccbd, provider auth, or release asset changes, add the relevant targeted tests or smoke commands before publishing.

## Publish Sequence

Use this order:

1. Commit release changes.
2. Push the branch.
3. Create and push tag `vX.Y.Z`.
4. Create the GitHub Release page for `vX.Y.Z`.
5. Let `Release Artifacts` upload assets.
6. Confirm `Release Artifacts` is green for the tag or a valid `workflow_dispatch` recovery, and confirm branch validation workflows for the release commit are green or consciously accepted as warnings:
   - `Tests`
   - `CCBD Real Platform Smoke`
   - `Cross-Platform Compatibility Test`
7. Confirm release assets exist:
   - `ccb-linux-x86_64.tar.gz`
   - `ccb-macos-universal.tar.gz`
   - `SHA256SUMS`

The current workflow expects the Release page to exist before uploading assets. If `Release Artifacts` fails with `release not found`, create the Release and rerun the workflow.

## Recovery Runbook

Use the checker output first; each FAIL includes a suggested fix. Common cases:

- Release page missing: create it with `gh release create vX.Y.Z --repo SeemSeam/claude_codex_bridge --title vX.Y.Z --notes-file <notes-file>`, then rerun `Release Artifacts`.
- Release Artifacts recovered through `workflow_dispatch`: run it with input `tag=vX.Y.Z`; the checker accepts this but warns if the workflow head SHA does not match the release tag.
- Release assets missing: rerun the `Release Artifacts` workflow for the tag, then verify `ccb-linux-x86_64.tar.gz`, `ccb-macos-universal.tar.gz`, and `SHA256SUMS`.
- Tag missing locally or remotely: stop and confirm the intended release commit before creating or pushing the tag.
- Tag SHA mismatch: do not force-push automatically; inspect the tag and ask for explicit maintainer approval before rewriting release history.
- GitHub CLI unauthenticated: run `gh auth login`, then rerun the published check.
- Workflow red: open the failed run, fix the root cause, rerun the workflow, and keep the release incomplete until it is green.
- README install URL mismatch: update both English and Chinese install snippets to the active public repo.
- Empty changelog or README release entry: add concrete user-facing bullets, not placeholder headings.

## Post-Release Verification

Run:

```bash
gh release view vX.Y.Z --repo SeemSeam/claude_codex_bridge --json tagName,url,assets
gh run list --repo SeemSeam/claude_codex_bridge --limit 10
git status --short --branch
```

Report only the useful facts: version, commit/tag, release URL, key fixes, test status, artifact status, and whether the worktree is clean.

## Stop Conditions

Do not call the release complete if any of these are true:

- README or README_zh still shows an old current version or stale current-release highlights.
- `VERSION`, `ccb`, changelog, badges, or release notes disagree.
- The release tag is missing, points to the wrong commit, or differs between local and origin.
- GitHub latest release does not point to the new tag after publish.
- Required release assets are missing.
- `SHA256SUMS` does not contain checksum entries for every required tarball asset.
- Tests or Release Artifacts failed.
- The worktree has uncommitted release edits.
