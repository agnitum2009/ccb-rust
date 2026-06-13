# Source Research

Date: 2026-06-13

## Kimi

Observed upstream:

- GitHub release source used for real validation:
  `MoonshotAI/kimi-cli` tag `1.47.0`.
- Earlier npm package probe: `@moonshot-ai/kimi-code@0.14.2`.
- Binary: `kimi`.
- Node engine: `>=22.19.0`.
- Official startup path: run `kimi` inside a project after installation and
  authenticate with `/login` or `kimi login`.
- CLI help also exposes `--prompt` for noninteractive prompt mode and
  `--output-format text|stream-json`.
- Kimi 1.47.0 exits with an error when launched with `--continue` in a workdir
  with no previous session, so CCB must not inject restore flags implicitly.
- Kimi 1.47.0 TUI echoes submitted prompt text before the assistant reply; CCB
  must ignore the prompt-echo `CCB_DONE` line and wait for the model's own
  done marker.
- Kimi 1.47.0 needs the TUI input area to be ready before CCB sends prompt
  text; prompt delivery immediately after pane creation can otherwise be
  printed before the welcome screen and not executed.
- Kimi 1.47.0 help exposes `--yolo`, `--yes`, `--auto-approve`, and `-y` for
  automatic approval. It rejects the older `--auto` flag at CLI parse time.
  CCB should inject `--auto-approve` for new Kimi versions while still
  recognizing user-provided `--auto` as an explicit legacy auto flag to avoid
  duplicate injection.
- Kimi 1.47.0 writes project-scoped turn evidence under
  `~/.kimi/sessions/<md5(project-path)>/<session>/wire.jsonl`.
- Observed Kimi native turn events include `TurnBegin` with `user_input`,
  `ContentPart` text chunks, `StatusUpdate` with `message_id`, and `TurnEnd`.
  CCB can bind on `CCB_REQ_ID` in `TurnBegin` and complete on `TurnEnd`.
- Source probe of npm package `@moonshot-ai/kimi-code@0.14.2` found a second
  event vocabulary in `dist/main.mjs`: `turn.started`, `assistant.delta`, and
  `turn.ended` with `reason=completed|cancelled|failed`. The package's
  `FileSystemAgentRecordPersistence` writes JSON records to `wire.jsonl`, but
  the real 1.47.0 binary observed locally writes the capitalized
  `TurnBegin`/`ContentPart`/`TurnEnd` wrapper shape. CCB therefore treats the
  1.47.0 shape as the primary observed contract and accepts the source-style
  event names as compatibility if they appear in a wire log.

First CCB slice:

- Register provider key `kimi`.
- Default executable is `kimi`.
- Override command with `KIMI_START_CMD`.
- Use interactive pane-backed runtime first so behavior matches other managed
  CCB agents.

## DeepSeek / Deep Code

Observed upstream:

- DeepSeek API docs list Deep Code as an open-source terminal AI coding
  assistant for DeepSeek-V4.
- GitHub: `lessweb/deepcode-cli`.
- Package: `@vegamo/deepcode-cli@0.1.29`.
- Binary: `deepcode`.
- Node engine: `>=22`.
- DeepSeek docs configure Deep Code through `~/.deepcode/settings.json` with
  `MODEL`, `BASE_URL`, and `API_KEY` under `env`.
- CLI help says `deepcode -p/--prompt` launches with a pre-filled prompt; it
  does not claim noninteractive completion output.
- DeepCode documents/ships project session persistence under
  `~/.deepcode/projects/<project-code>/sessions-index.json` plus
  `<session-id>.jsonl`.
- Observed DeepCode status values include completed and non-terminal states;
  CCB can bind by the user jsonl message containing `CCB_REQ_ID` and complete
  on native `status=completed`.
- Source probe of `@vegamo/deepcode-cli@0.1.29` confirmed the status set:
  `pending`, `processing`, `completed`, `failed`, `interrupted`,
  `ask_permission`, `waiting_for_user`, and `permission_denied`.
  `denySessionPermission()` updates a session entry to
  `status=permission_denied` with a `failReason`, so CCB should terminalize
  that state with diagnostics instead of waiting for timeout.

First CCB slice:

- Register provider key `deepseek`.
- Default executable is `deepcode`.
- Override command with `DEEPSEEK_START_CMD`.
- Do not auto-create or fetch API keys.

## AGY / Antigravity

Observed local storage:

- Local binary: `/home/bfly/.local/bin/agy`, version `1.0.7`.
- CLI help exposes `--print`, `--prompt-interactive`, `--conversation`,
  `--continue`, and `--print-timeout`.
- Antigravity writes transcript logs under
  `~/.gemini/antigravity-cli/brain/<conversation>/.system_generated/logs/`.
- Transcript jsonl rows include `source`, `type`, `status`, `created_at`, and
  `content`.
- CCB can bind by `USER_EXPLICIT` / `USER_INPUT` rows containing `CCB_REQ_ID`
  and complete from `MODEL` response rows such as `PLANNER_RESPONSE` with
  `status=DONE`.
- Local transcript inventory found only redaction-safe event triples such as
  `USER_EXPLICIT/USER_INPUT/DONE`, `MODEL/PLANNER_RESPONSE/DONE`,
  `MODEL/RUN_COMMAND/DONE`, and `MODEL/VIEW_FILE/DONE`.
- Antigravity also stores sqlite conversation databases under
  `~/.gemini/antigravity-cli/conversations/<conversation>.db`. The observed
  `steps` table uses numeric `step_type` and `status` values; because the
  stable meaning of those enums is not source-confirmed, transcript jsonl
  remains the primary CCB completion authority and sqlite is only a possible
  future diagnostic aid.

## OpenCode

Observed local/package boundary:

- Local `opencode --version` returned `1.16.2`.
- Package `opencode-ai@1.16.2` is a small npm installer/binary wrapper
  (`bin/opencode.exe`, `postinstall.mjs`) rather than directly reviewable
  application source.
- CCB already has an authoritative native completion contract in
  `docs/opencode-completion-contract.md`: `CCB_DONE`, terminal quiet time, and
  pane text are not completion authority; a matched assistant message is
  complete only when OpenCode structured storage records `time.completed`.
- No change is needed for the current native pivot beyond keeping tests/docs
  from reintroducing pane marker completion for OpenCode.

## Local Probe Evidence

- Local Node: `v22.20.0`.
- Installed real Kimi release:
  `/home/bfly/.local/bin/kimi --version` returned `kimi, version 1.47.0`.
- `npm view @moonshot-ai/kimi-code@0.14.2 version bin engines --json` returned
  bin `kimi` and engine `>=22.19.0`.
- `npm view @vegamo/deepcode-cli@0.1.29 version bin engines --json` returned
  bin `deepcode` and engine `>=22`.
- `npx --yes @moonshot-ai/kimi-code@0.14.2 --help` succeeded.
- `npx --yes @vegamo/deepcode-cli@0.1.29 --help` succeeded.
- Extracted npm package tarballs under `/tmp/ccb-native-src-probe` for local
  source inspection.
- Local AGY transcript/db inventory confirmed transcript completion evidence
  without printing user content.
- `npm pack opencode-ai@1.16.2` confirmed OpenCode's npm package exposes only
  installer/binary wrapper files, so the existing CCB storage contract remains
  the local source of truth for OpenCode completion behavior.
- `kimi --auto-approve --version` succeeded on local Kimi 1.47.0, while
  `kimi --auto --version` failed with "No such option: --auto".
