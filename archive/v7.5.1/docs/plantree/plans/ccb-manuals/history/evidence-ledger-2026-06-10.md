# Evidence Ledger - 2026-06-10

Plan: CCB manuals

## Archi And Hippos

Working directory:

- `/home/bfly/yunwei/ccb_source`

Commands run:

```bash
archi --help
archi --version
archi --check .
archi --refresh-from-hippos --allow-static --out .architec/manual-architecture-analysis-20260610.json .
```

Observed tool identity:

- `archi` path: `/home/bfly/.nvm/versions/node/v22.20.0/bin/archi`.
- Real path:
  `/home/bfly/.nvm/versions/node/v22.20.0/lib/node_modules/@seemseam/archi/bin/archi.js`.
- Version: `0.2.16`; latest reported version also `0.2.16`.

Observed artifact state:

- Current Hippos bundle: `.hippos/bundle-state.json`.
- Current generated timestamp: `2026-06-10T13:56:37.185325+00:00`.
- Current fingerprint:
  `4f4df01215599f15ac0c484bce6b0dbc9f60b6717afe03dddaa2ac4cd1e540e8`.
- Current counts: 1556 indexed files, 1869 manifest files, 1153 signature
  files.
- Legacy bundle: `.hippocampus/bundle-state.json`.
- Legacy generated timestamp: `2026-05-29T17:16:11.137537+00:00`.
- Legacy fingerprint:
  `5c9790bdee13eb77e00a4e7780c68b7188c8cf85ab6a7f03c90ca0f636388c41`.
- Legacy counts: 1518 indexed files, 1819 manifest files, 1121 signature
  files.

Conclusion:

- `archi --check .` was not stale; the current refresh target is `.hippos/`.
- `.hippocampus/` should be treated as legacy evidence unless a chapter
  explicitly compares old and new generated maps.
- `.architec/manual-architecture-analysis-20260610.json` is accepted as
  advisory evidence for planning, not as the final full architecture map.

Manual consumers:

- Developer manual architecture overview.
- Developer manual generated-code-map appendix.
- Developer manual communication chapter source-navigation notes.

## Communication Source Inventory

Plan artifact:

- `topics/communication-source-inventory.md`

Source areas read:

- `lib/cli/parser_runtime/ask.py`
- `lib/cli/services/ask.py`
- `lib/cli/services/ask_runtime/submission.py`
- `lib/ccbd/handlers/submit.py`
- `lib/ccbd/api_models_runtime/messages.py`
- `lib/ccbd/services/dispatcher_runtime/`
- `lib/message_bureau/`
- `lib/mailbox_kernel/`

Accepted finding:

- The central communication path is:
  CLI ask parsing -> `MessageEnvelope` -> ccbd submit handler -> dispatcher
  submission plan -> job queue/state -> message-bureau records -> provider
  execution -> polling/finalization -> reply/inbound event projection.
- Callback is modeled as persisted parent/child/continuation linkage, not a
  synchronous wait.

Manual consumers:

- Developer manual communication logic chapter.
- User manual ask/watch/pend/queue/inbox/trace chapter.

## Command And Config Source Inventory

Plan artifact:

- `topics/command-config-inventory.md`

Source areas read:

- `lib/cli/entrypoint_runtime.py`
- `lib/cli/parser.py`
- `lib/cli/parser_runtime/constants.py`
- `lib/cli/parser_runtime/commands.py`
- `lib/cli/parser_runtime/fault.py`
- `lib/cli/ask_usage.py`
- `lib/cli/roles_runtime/commands.py`
- `lib/cli/tools_runtime/neovim.py`
- `lib/agents/config_loader_runtime/common.py`
- `lib/agents/config_loader_runtime/io_runtime/documents.py`
- `lib/agents/config_loader_runtime/parsing_runtime/validation.py`
- `lib/agents/config_loader_runtime/parsing_runtime/topology.py`
- `lib/agents/config_loader_runtime/parsing_runtime/agent_specs.py`

Accepted finding:

- CLI reference work should be grouped by dispatch layer rather than by a flat
  alphabetical command list.
- Config reference work should be grouped by document shape, topology mode,
  agent spec, provider profile, UI/sidebar, tool windows, and maintenance.
- Role loading is part of both command and config systems: role commands
  install/update/sync/add rolepacks, while config loading expands role ids into
  project agent specs before runtime mounting.

Manual consumers:

- Developer manual config and rolepack chapters.
- User manual command reference.
- User manual configuration reference.

## Manual Builds

Developer manual source:

- `docs/manuals/developer-guide/`

Developer manual build command:

```bash
make
```

Build directory:

- `docs/manuals/developer-guide/build/`

Observed PDF metadata:

- Title: `CCB 开发说明书`
- Output: `docs/manuals/developer-guide/build/main.pdf`
- Page size: A4
- Pages: 82
- File size: 529390 bytes

User manual source:

- `docs/manuals/user-guide/`

User manual build command:

```bash
make
```

Build directory:

- `docs/manuals/user-guide/build/`

Observed PDF metadata:

- Title: `CCB 使用说明书`
- Output: `docs/manuals/user-guide/build/main.pdf`
- Page size: A4
- Pages: 38
- File size: 308714 bytes

Source volume:

- Non-build files under both manual source trees: 4125 lines.
- Generated build directories are ignored by local `.gitignore` files in each
  manual directory.

Style revision:

- Added shared style source at `docs/manuals/manual-style.tex`.
- Replaced the default title pages with manual-specific title pages.
- Added restrained blue/gray headings, stable page headers, CJK slant fallback,
  syntax-colored code blocks, line numbers, and safer inline source/command
  wrapping.
- Verified command inline text preserves spaces, e.g. `config validate`,
  `doctor ps`, and `roles install`.
- Build logs were checked for `Overfull`, `Font Warning`, `undefined`,
  `Warning`, `Emergency`, and `Runaway`; none remained after the style pass.

## Reviewer1 User Manual Review

Reviewer request:

- `job_8906713f6caf`

Artifact:

- `.ccb/ccbd/artifacts/text/completion-reply/job_8906713f6caf-art_4cc0a0677a114bcb.txt`
- SHA256:
  `72fcb60cb8cced7db8447abec8ec1ea9e392630f3ec2b5c7b8e5978f3712288b`

Accepted findings and changes:

- Fixed `ccb maintenance schedule`: it is now documented as
  `ccb maintenance schedule --after <duration> [--reason TEXT]`, not as a
  read-only schedule view.
- Documented `ccb maintenance tick --force` and
  `ccb maintenance tick --no-dispatch`.
- Documented that `ccb maintenance enable` and
  `ccb maintenance disable` currently return a config-authority hint; users
  should edit `[maintenance.heartbeat].enabled`.
- Moved version 1 discussion out of the main config path into a historical
  supplement. The main config chapter now leads with current `version = 2`
  semantics.
- Renamed "legacy layout" user-facing text to "基础布局模式".
- Added `roles add` option flags to the role command table.
- Strengthened `ccb clear` and `ccb restart` safety-boundary descriptions.
- Added a command quick-reference section for diagnostics, maintenance,
  config validation, and fault injection.
- Added an operation-recipe note warning users not to use the source checkout
  as the runtime validation project.

Verification:

- `make` in `docs/manuals/user-guide` completed successfully.
- `pdfinfo docs/manuals/user-guide/build/main.pdf` reported 38 pages and
  308714 bytes.
- User-manual build log check found no `Overfull`, `Font Warning`,
  `undefined`, `Warning`, `Emergency`, or `Runaway` diagnostics.
- `pdftotext` checks confirmed `legacy layout` is no longer present, version 1
  appears only in the historical supplement, and reviewed maintenance command
  text is present.

## CCB Self Expert Markdown Guide

Guide source:

- `docs/manuals/ccb-self-expert-guide.md`

Purpose:

- Provide a Markdown operating manual for `agentroles.ccb_self` so the role can
  act as a CCB architecture, usage, config, communication, diagnosis, and
  bounded recovery expert.

Source inputs:

- `docs/plantree/plans/ccb-self-role/README.md`
- `docs/plantree/plans/ccb-self-role/topics/ccb-expert-knowledge-role.md`
- `docs/plantree/plans/ccb-self-role/topics/recovery-runbooks.md`
- `docs/plantree/plans/ccb-manuals/topics/communication-source-inventory.md`
- `docs/plantree/plans/ccb-manuals/topics/command-config-inventory.md`
- existing developer and user manual source trees under `docs/manuals/`

Accepted content:

- Role boundary: `ccb_self` is a CCB expert and maintenance assistant, not
  `ccbd`, keeper, lifecycle authority, or business-task owner.
- Authority hierarchy: config and mounted daemon graph are separated from
  dispatcher/message records, runtime evidence, panes, provider sessions, and
  residue.
- Runtime safety: source validation for this repository stays outside
  `ccb_source` and uses the dedicated `ccb_test` wrapper from
  `/home/bfly/yunwei/test_ccb2`.
- Config guidance: current `version = 2` grammar is primary; version 1 is not
  part of the main operating path; `[windows]` topology and basic layout mode
  are distinguished.
- Command guidance: ask/callback/artifacts, observer commands, lifecycle
  commands, diagnostics, maintenance, role/tool commands, and removed commands
  are summarized from parser and source inventories.
- Communication guidance: the guide explains the ask path, `MessageEnvelope`,
  dispatcher versus mailbox state, finalization, callback, artifacts, retry,
  resubmit, cancel, and ack.
- Recovery guidance: the guide gives bounded playbooks for missing replies,
  blocked queues, missing panes, corrupted contexts, provider failures, config
  drift, and `ccb_self` itself being broken.

Verification:

- Markdown source was checked for trailing whitespace.
- `wc -l docs/manuals/ccb-self-expert-guide.md` reported 1052 lines.
- Manual index `docs/manuals/README.md` links the new guide.
- The guide is intentionally role-facing Markdown and does not require a PDF
  build.
