# Integration Design

Date: 2026-06-13

## Provider Keys And Commands

| Provider key | Default command | Override env |
| :--- | :--- | :--- |
| `kimi` | `kimi` | `KIMI_START_CMD` |
| `deepseek` | `deepcode` | `DEEPSEEK_START_CMD` |

The `deepseek` provider key follows user intent and model family language; the
actual CLI command remains `deepcode` because that is the DeepSeek documented
terminal integration.

## Runtime Model

Both providers enter CCB as optional built-in pane-backed providers:

- `ProviderManifest` uses `SESSION_BOUNDARY`.
- `kimi` uses `CompletionSourceKind.SESSION_EVENT_LOG`.
- `deepseek` uses `CompletionSourceKind.SESSION_SNAPSHOT`.
- `ProviderRuntimeLauncher` uses `simple_tmux`.
- `ProviderSessionBinding` uses `.kimi-session` and `.deepseek-session`.
- Startup command supports `spec.startup_args`, `spec.env`, caller context env,
  and `provider_command_template`.

## Completion Strategy

The current strategy uses provider-native session/event stores:

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
7. Completed-native-empty replies are `incomplete` with
   `empty_provider_reply` diagnostics, not `completed`.
8. Missing anchors and long-running native turns terminalize with explicit
   provider-native timeout or anchor-missing reasons.

## Config Boundary

Supported first-slice config:

```toml
[windows]
main = "kimi_agent:kimi, deep_agent:deepseek"

[agents.kimi_agent]
provider = "kimi"

[agents.deep_agent]
provider = "deepseek"
```

Not supported in first slice:

- `key` / `url` shortcuts for Kimi or DeepSeek.
- Automatic writing of `~/.deepcode/settings.json`.
- Automatic Kimi login.

## Tests

Focused unit tests should cover:

- Optional provider registry includes `kimi` and `deepseek`.
- Runtime specs include `.kimi-session` and `.deepseek-session`.
- Start command env overrides and default executables.
- Session binding maps and runtime launcher maps include both providers.
- Native readers parse Kimi `wire.jsonl`, DeepCode sessions, and AGY
  transcripts.
- Provider adapters emit `SESSION_ROTATE`, `ANCHOR_SEEN`, `ASSISTANT_FINAL`,
  and `TURN_BOUNDARY` from native evidence.
- Provider adapters diagnose completed-native-empty replies and fail on missing
  runtime state.
- Config loader accepts agents using `provider = "kimi"` and
  `provider = "deepseek"`.

Source-runtime validation should run from `/home/bfly/yunwei/test_ccb2` using
`/home/bfly/yunwei/ccb_source/ccb_test` and isolated source home. Real CLI
help/version checks validate installability; CCB ask completion can use
provider command templates that point to deterministic stub TUIs when API
credentials are unavailable.
