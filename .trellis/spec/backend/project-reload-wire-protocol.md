# Project Reload Wire Protocol

## Scenario: reload `.ccbr/ccbr.config` through ccbrd

### 1. Scope / Trigger

- Trigger: any change to `project_reload_config`, reload dry-run/apply planning, service-graph publish, runtime registry synchronization, or additive reload blockers.
- Reference owner: Python `backup/python-reference/lib/ccbd/handlers/project_reload.py` and `project_reload_payload.py`.
- Runtime owner: Rust `ccbrd` reload apply service, project namespace, agent registry, and dispatcher read models.

### 2. Signatures

- RPC op: `project_reload_config`
- Payload: `{ "dry_run": true|false }`; truthy strings such as `"true"` are accepted.
- Dry-run response: reload plan record with `dry_run: true`.
- Apply response: flattened apply record with `dry_run: false`, `mutation_enabled`, `safe_to_apply`, `future_safe_to_apply`, `operations`, `drain_intents`, `namespace_patch_plan`, `reasons`, `warnings`, and `errors`.

### 3. Contracts

- Dry-run must not mutate app config, namespace, registry, or dispatcher state.
- Non-dry-run invalid config returns the Python error shape: `status: invalid_config`, `dry_run: false`, `mutation_enabled: false`, `safe_to_apply: false`, and `diagnostics.reason: invalid_config`.
- Non-dry-run no-change returns a non-mutating apply payload, not a local Rust `applied=true` record.
- Successful apply returns `status: published`, `stage: publish_transaction`, `mutation_enabled: true`, and `safe_to_apply: true`.
- Blocked/failed apply returns `mutation_enabled: false`, `safe_to_apply: false`, and `errors` derived from diagnostics.
- Published config must update the Rust runtime read models (`current_config`, registry, dispatcher agent list) so follow-up `project_view`, `queue`, and `ask` read the new agent set.
- Remove-agent apply must block before namespace mutation if the target agent has outstanding dispatcher work or is `busy`/`running`/`active`.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| `dry_run=true` | Return plan only; no runtime mutation |
| Invalid config, dry run | `status=invalid_config`, `dry_run=true`, non-mutating |
| Invalid config, apply | `status=invalid_config`, `dry_run=false`, diagnostics reason `invalid_config` |
| Add agent apply | `status=published`; registry and dispatcher include the agent |
| Remove idle agent apply | `status=published`; registry and dispatcher remove the agent |
| Remove busy/running/active agent apply | `status=blocked`; diagnostics reason `agent_busy`; registry unchanged |
| Unsupported replace-agent apply | `status=blocked`; diagnostics reason `unsupported_plan_class` or `plan_not_future_safe` |

### 5. Good / Base / Bad Cases

- Good: after a published add-agent reload, `project_view` and `queue` can see the newly configured agent without restarting ccbrd.
- Base: a blocked reload explains the blocker and leaves runtime read models unchanged.
- Bad: handler returns Rust-local `{status:"ok", applied:true}` while Python clients expect `published/blocked/noop` payload fields.

### 6. Tests Required

- Unit: `non_dry_run_apply_payload_matches_python_reload_shape`.
- Unit: `published_reload_payload_is_marked_mutating_without_errors`.
- Integration: `cargo test -p ccbr-daemon --test reload_tests -- --test-threads=1`.
- Package: `cargo test -p ccbr-daemon -- --test-threads=1`.

### 7. Wrong vs Correct

#### Wrong

```json
{ "status": "ok", "applied": true, "added_agents": ["agent2"] }
```

#### Correct

```json
{
  "status": "published",
  "dry_run": false,
  "mutation_enabled": true,
  "safe_to_apply": true,
  "operations": [{"op": "add_agent", "agent": "agent2"}],
  "errors": []
}
```
