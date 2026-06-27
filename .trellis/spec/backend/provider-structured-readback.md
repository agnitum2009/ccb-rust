# Provider Structured Readback

## Scenario: provider completion readback and pane fallback isolation

### 1. Scope / Trigger

- Trigger: any change to provider polling, Claude/Codex JSONL readers, daemon pane capture, inbox reply extraction, or provider runtime-state keys that carry completion text.
- Reference owners:
  - Claude project logs: `~/.claude/projects/<project-key>/*.jsonl` where `<project-key>` preserves a leading slash as `-` (for example `/mnt/d/dapro-ass` -> `-mnt-d-dapro-ass`).
  - Codex session JSONL: the named `.ccbr/.codex-<agent>-session` binding.
- Runtime owner: `ccbrd` provider execution plus `ccbr-providers` polling.
- Hard rule: do not disable, remove, skip, or mask Codex hooks to make readback easier.

### 2. Signatures

- Daemon active-pane capture state key: `pane_text_buffer`.
- Structured reply state keys: `reply_buffer`, `last_agent_message`, provider-specific log reader state such as `session_path`, `claude_projects_root`, `state.log_path`, and `state.offset`.
- Claude project key function: map every non-ASCII-alphanumeric character to `-`; do not trim leading or trailing dashes.
- Provider polling output: terminal `ProviderPollDecision` with clean final reply text, not raw pane text.

### 3. Contracts

- `reply_buffer` is reserved for provider-owned structured reply text. Daemon tmux capture must never overwrite it.
- Daemon tmux capture writes only `pane_text_buffer` for best-effort fallback detection.
- If a structured log binding exists (`session_path`, `claude_projects_root`, or provider-specific log path), provider polling must prefer that log and must not terminalize from pane text.
- Pane fallback is allowed only when no structured log source is expected or available for that submission.
- Inbox-visible replies must not contain original prompts, TUI chrome, progress spinners, hook output, or terminal prompt decorations.
- When a completion-tracker decision terminalizes a job, provider execution state must also be finished so active-only polling stops.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Claude project root configured | Poll Claude JSONL under the exact project key; do not trim the leading `-` |
| Structured log exists but has no terminal assistant event | Keep job running; do not complete from `pane_text_buffer` |
| Structured log has final assistant text | Complete with that text, ignoring dirty pane text |
| No structured log binding exists | Pane fallback may complete only after provider-specific prompt/anchor readiness checks |
| Daemon captures pane text | Runtime state updates `pane_text_buffer`, not `reply_buffer` |
| Completion tracker returns terminal decision | Remove the job from active provider execution state |

### 5. Good / Base / Bad Cases

- Good: Claude writes `CCBR_SMOKE_SUBMIT_*` to its JSONL and the sender inbox shows exactly that token.
- Base: pane text contains `Reply exactly: TOKEN` plus a spinner while JSONL is still pending; the job remains running.
- Bad: inbox reply includes the original prompt, provider TUI chrome, or startup/hook text because pane capture overwrote `reply_buffer`.

### 6. Tests Required

- Unit: Claude project key preserves leading dash for absolute project paths.
- Unit: daemon active-pane capture stores `pane_text_buffer` separately from provider `reply_buffer`.
- Unit: Claude pane fallback returns `None` when structured logs are expected.
- Unit: structured Claude completion ignores dirty `pane_text_buffer` and returns `last_agent_message` / final assistant text.
- Unit: Codex pane fallback reads `pane_text_buffer` first, with legacy `reply_buffer` only as compatibility fallback.
- Live smoke: `ccbr ask <target> --from <sender> "Reply exactly: TOKEN"` then `ccbr inbox --detail <sender>` contains only `TOKEN`.

### 7. Wrong vs Correct

#### Wrong

```text
feed_active_pane_text_to_execution -> runtime_state["reply_buffer"] = capture-pane text
provider poll -> terminal reply from reply_buffer while structured logs are configured
```

#### Correct

```text
feed_active_pane_text_to_execution -> runtime_state["pane_text_buffer"] = capture-pane text
provider poll -> structured JSONL final assistant text wins; pane fallback only when no structured log source is expected
```

## Scenario: shared provider asset cache is concurrency-safe

### 1. Scope / Trigger

- Trigger: any change to Codex/Claude provider profile materialization, projected asset routing, shared plugin/cache bundle paths, or parallel agent startup.
- Owner: provider-core projected asset cache must be safe when multiple agents start at the same time.
- Hard rule: do not disable Codex hooks or remove inherited plugin assets to avoid cache races.

### 2. Signatures

- Cache API: `copy_projected_tree_to_cache(source: &Path, bundle_root: &Path, label: &str) -> Result<bool>`.
- Caller API: `ensure_shared_tree_bundle(source: &Path, bundle_root: &Path) -> Option<PathBuf>`.
- Marker file: `<bundle_root>.ccbr-projection.json` on ccbr, `<bundle_root>.ccb-projection.json` on ccb-legacy.

### 3. Contracts

- Each writer must copy into a unique temp directory before publishing the bundle.
- If another writer already published a bundle with the required entries, later writers must treat that as success and remove only their own temp directory.
- A valid existing bundle must not be deleted just because another writer lost the publish race.
- The cache may remove and replace `bundle_root` only when the existing bundle is still invalid after the race check.
- Claude project-key test helpers must mirror the reader contract: map every non-ASCII-alphanumeric char to `-`; do not trim leading dashes.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Multiple agents materialize the same Codex plugin bundle | All callers return `Ok(true)` |
| Loser sees `DirectoryNotEmpty` / existing target on publish | Re-check required entries, then succeed if valid |
| Bundle is invalid before publish | Remove/replace only after the validity re-check fails |
| Absolute Claude project path like `/mnt/d/repo` | Key remains `-mnt-d-repo`; do not trim the leading dash |

### 5. Good / Base / Bad Cases

- Good: three agents launch concurrently and inherited Codex plugins are visible without `destination already exists`.
- Base: a single agent creates the shared bundle and marker.
- Bad: provider launch fails because two agents share `.bundle.tmp`, or a losing writer deletes a valid bundle.

### 6. Tests Required

- Unit: `cargo test -p ccbr-provider-core projected_assets -- --test-threads=1`.
- Provider profile: `cargo test -p ccbr-provider-profiles test_materialize_codex_profile_routes_plugins_through_shared_bundle -- --test-threads=1`.
- Claude reader/tests: `cargo test -p ccbr-providers --test provider_claude_tests -- --test-threads=1`.
- ccb-legacy equivalent where shared provider-core exists: run the matching `ccb-*` tests in `/tmp/ccb-legacy-sync`.

### 7. Wrong vs Correct

#### Wrong

```text
tmp = ".bundle.tmp"
remove(bundle_root)
rename(tmp, bundle_root)
```

#### Correct

```text
tmp = unique_tmp_tree_path(bundle_root)
copy(source, tmp)
if bundle_root now has required entries: remove(tmp); success
else try rename(tmp, bundle_root)
if rename loses the race: re-check bundle_root before any remove
```

## Scenario: sealed providers stay out of default registries

### 1. Scope / Trigger

- Trigger: any provider is explicitly sealed by owner decision after upstream/source intake.
- Owner: default provider discovery and execution registries.

### 2. Signatures

- Default catalog: `OPTIONAL_PROVIDER_NAMES`.
- Default execution/backend registries: `build_default_execution_registry()` and `build_default_backend_registry()`.
- Runtime/client maps: `RUNTIME_SPECS_BY_PROVIDER` and `CLIENT_SPECS_BY_PROVIDER`.

### 3. Contracts

- A sealed provider may keep archived source/tests for rollback, but must not appear in default provider discovery, runtime/client spec maps, or execution/backend registries.
- Sealed providers are not live-acceptance blockers until explicitly unsealed by owner decision.
- Current sealed provider: `zai`.
- Current non-mobile live provider acceptance scope: `codex`, `kimi`, and `claude`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| Default optional provider discovery runs | `zai` is absent |
| Default runtime/client spec maps are queried | `zai` is absent |
| Default execution/backend registries are built | `zai` is absent |
| Archived ZAI module tests exist | They may remain, but do not imply default support |

### 5. Good / Base / Bad Cases

- Good: Codex/Kimi/Claude are accepted; ZAI is absent from defaults.
- Base: ZAI code remains for future unseal/rollback.
- Bad: default registry advertises ZAI after owner sealed it.

### 6. Tests Required

- Unit: provider-core registry tests assert sealed providers are absent from defaults.
- Unit: provider runtime/client map tests assert sealed providers are absent from default maps.
- Unit: provider execution/backend registry tests assert sealed providers are absent from defaults.

### 7. Wrong vs Correct

#### Wrong

```text
Upstream has provider_backends/zai, so ccbr defaults advertise zai.
```

#### Correct

```text
Owner sealed zai; source may remain archived, but default registries omit it.
```
