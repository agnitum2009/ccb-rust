# Integration Design

Date: 2026-06-13

## Provider Keys And Commands

| Provider key | Default command | Override env |
| :--- | :--- | :--- |
| `kimi` | `kimi` | `KIMI_START_CMD` |
| `deepseek` | `deepcode` | `DEEPSEEK_START_CMD` |
| `mimo` | `mimo` | `MIMO_START_CMD` |

The `deepseek` provider key follows user intent and model family language; the
actual CLI command remains `deepcode` because that is the DeepSeek documented
terminal integration.

## Runtime Model

These providers enter CCB as optional built-in managed providers:

- `kimi` and `deepseek` use `ProviderManifest` `SESSION_BOUNDARY`.
- `kimi` uses `CompletionSourceKind.SESSION_EVENT_LOG`.
- `deepseek` uses `CompletionSourceKind.SESSION_SNAPSHOT`.
- `mimo` uses `CompletionFamily.STRUCTURED_RESULT` and
  `CompletionSourceKind.STRUCTURED_RESULT_STREAM` for ask execution because
  terminalization comes from `mimo run --format json` result events.
- `ProviderRuntimeLauncher` uses `simple_tmux`.
- `ProviderSessionBinding` uses `.kimi-session`, `.deepseek-session`, and
  `.mimo-session`.
- Startup command supports `spec.startup_args`, `spec.env`, caller context env,
  and `provider_command_template`.

## Completion Strategy

The current strategy uses provider-native session/event stores or structured
result streams:

1. Send a wrapped prompt to the managed provider pane.
2. The prompt contains `CCB_REQ_ID: <job_id>`.
3. Do not ask Kimi, DeepSeek/DeepCode, or AGY to print `CCB_DONE`.
4. Kimi polls `wire.jsonl`, binds the turn by `CCB_REQ_ID`, emits
   `ASSISTANT_FINAL` from `ContentPart`, and emits `TURN_BOUNDARY` on
   native `TurnEnd`.
5. DeepSeek polls DeepCode `sessions-index.json` and session jsonl, binds the
   user message by `CCB_REQ_ID`, emits `ASSISTANT_FINAL` from assistant
   messages, and emits `TURN_BOUNDARY` on native `status=completed`.
6. AGY polls Antigravity transcript logs, binds `USER_INPUT` by `CCB_REQ_ID`,
   emits `ASSISTANT_FINAL` from model response events, and emits
   `TURN_BOUNDARY` when a completed response is observed.
7. MiMo asks run as native subprocesses using
   `mimo run --format json --dir <workdir>`. CCB emits `ASSISTANT_FINAL` from
   nested `part.text` events and emits `TURN_BOUNDARY` / terminal completed on
   `step_finish` with `part.reason=stop`.
8. Completed-native-empty replies are `incomplete` with
   `empty_provider_reply` diagnostics, not `completed`.
9. Missing anchors and long-running native turns terminalize with explicit
   provider-native timeout or anchor-missing reasons.

## Skill And Instruction Injection

Provider onboarding must include a capability-projection check in addition to
native completion detection:

- If the provider exposes native skills, use that native surface.
- If the provider exposes only instruction files/config, inject CCB ask guidance
  through that instruction surface.
- Do not ask the model to rediscover `ask` usage from memory alone when a
  provider-native or provider-supported projection path exists.

Current behavior:

- Kimi gets inherited CCB ask skill content from
  `inherit_skills/kimi_skills/ask/SKILL.md`. Startup materializes a managed
  skills root under `.ccb/agents/<agent>/provider-state/kimi/inherited-skills`
  and passes it to Kimi with `--skills-dir`. Because Kimi treats any
  `--skills-dir` as replacement for default discovery, CCB first passes
  existing default Kimi project/user skill directories, then appends managed
  inherited and role skill roots.
- OpenCode does not expose a stable `--skills-dir` equivalent in the observed
  CLI help. CCB writes `.ccb/runtime/skills/<agent>/opencode/ask.md` and appends
  that path to generated `opencode.json.instructions` alongside the memory
  bridge.
- MiMo writes `.ccb/runtime/skills/<agent>/mimo/ask.md` and appends that path
  to generated `mimocode.json.instructions` alongside the memory bridge.
- `inherit_skills = false` disables inherited skill projection. For OpenCode,
  `inherit_memory = false` disables only the memory bridge; inherited ask
  instructions continue unless `inherit_skills = false` is also set.

## Config Boundary

Supported first-slice config:

```toml
[windows]
main = "kimi_agent:kimi, deep_agent:deepseek, mimo_agent:mimo"

[agents.kimi_agent]
provider = "kimi"

[agents.deep_agent]
provider = "deepseek"

[agents.mimo_agent]
provider = "mimo"
```

Not supported in first slice:

- `key` / `url` shortcuts for Kimi or DeepSeek.
- `key` / `url` shortcuts for MiMo.
- Automatic writing of `~/.deepcode/settings.json`.
- Automatic Kimi login.

## Tests

Focused unit tests should cover:

- Optional provider registry includes `kimi`, `deepseek`, and `mimo`.
- Runtime specs include `.kimi-session`, `.deepseek-session`, and
  `.mimo-session`.
- Start command env overrides and default executables.
- Kimi startup includes existing default skill directories and materialized CCB
  skill directories as repeatable `--skills-dir` arguments, while skipping
  missing directories.
- OpenCode generated config preserves user instructions and appends memory and
  ask-skill instruction entries without duplication.
- MiMo generated config preserves user instructions and appends memory and
  ask-skill instruction entries without duplication.
- Session binding maps and runtime launcher maps include the native providers.
- Native readers parse Kimi `wire.jsonl`, DeepCode sessions, and AGY
  transcripts.
- MiMo execution parses `mimo run --format json` nested `part.text` and
  `part.reason=stop`.
- Provider adapters emit `SESSION_ROTATE`, `ANCHOR_SEEN`, `ASSISTANT_FINAL`,
  and `TURN_BOUNDARY` from native evidence.
- Provider adapters diagnose completed-native-empty replies and fail on missing
  runtime state.
- Config loader accepts agents using `provider = "kimi"` and
  `provider = "deepseek"` and `provider = "mimo"`.

Source-runtime validation should run from `/home/bfly/yunwei/test_ccb2` using
`/home/bfly/yunwei/ccb_source/ccb_test` and isolated source home. Real CLI
help/version checks validate installability; CCB ask completion can use
provider command templates that point to deterministic stub TUIs when API
credentials are unavailable.
