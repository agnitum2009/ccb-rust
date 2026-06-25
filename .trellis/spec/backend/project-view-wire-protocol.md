# Project View Wire Protocol

## Scenario: sidebar reads ccbrd ProjectView

### 1. Scope / Trigger

- Trigger: any change to `project_view`, `project_view_dismiss_comms`, sidebar row grouping, sidebar panel sizing, comms rendering, or red-X workspace shutdown behavior.
- Reference owner: Python `backup/python-reference/lib/ccbd/project_view/service.py`.
- Consumer owner: Rust `tools/ccb-agent-sidebar/src/model.rs` and `tools/ccb-agent-sidebar/src/tui.rs`.

### 2. Signatures

- RPC op: `project_view`
- Payload: `{ "schema_version": 1 }`
- Response: `{ "view": ProjectView, "cache": ProjectViewCache }`
- RPC op: `project_view_dismiss_comms`
- Payload: `{ "id": "<comms id>" }` or `{ "comms_id": "<comms id>" }`
- Response: `{ "status": "dismissed", "id": "<comms id>", "dismissed_count": <number> }`

### 3. Contracts

- `project_view` response is top-level `{ "view": ..., "cache": ... }`.
- `view.schema_version` must be `1`.
- `cache.ttl_ms` must be positive so sidebar refresh backoff can use daemon-provided timing.
- `view.project` includes `id`, `root`, and `display_name`.
- `view.ccbd` includes `state`, `health`, `generation`, and `last_heartbeat_at`.
- `view.namespace` includes `epoch`, `socket_path`, `session_name`, `active_window`, `active_pane_id`, `entry_window`, and `sidebar.view`.
- `namespace.sidebar.view` includes `agents_height`, `comms_height`, `tips_height`, `comms_limit`, `comms_compact`, `tips_enabled`, and `tips`.
- Each window row includes `name`, `label`, `kind`, `order`, `active`, `tmux_window_id`, `tmux_window_index`, `sidebar_pane_id`, and `agents`.
- Tool windows are represented as `kind = "tool"` with an empty `agents` list.
- Each agent row includes `name`, `provider`, `window`, `order`, `pane_id`, `active`, `activity_state`, `activity_source`, `activity_reason`, `activity_symbol`, `activity_color`, `current_job_id`, and `queue_depth`.
- Each comms row includes `id`, `short_id`, `sender`, `target`, `status`, `business_status`, `status_label`, `body_preview`, `reply_status`, `reply_delivery_job_id`, `callback`, `short_reason`, `recoverable`, `recover_target`, and `block_reason`.
- The sidebar red X is a complete workspace exit. It must call `ccb shutdown`, not the legacy `ccb kill` action.

### 4. Validation & Error Matrix

| Condition | Expected behavior |
|-----------|-------------------|
| `schema_version` missing | Treat as version `1` for legacy clients |
| `schema_version != 1` | Return an error; do not silently change shape |
| No mounted namespace | Return empty/unknown namespace fields, but keep `{view, cache}` and sidebar defaults |
| Config has tool windows | Emit `kind = "tool"` rows with `agents = []` |
| Runtime entry is stopped | Do not render it as a live agent row |
| Job is accepted/queued | Agent activity is `pending`; comms status label is `send` |
| Job is running | Agent activity is `active`; comms status label is `work` |

### 5. Good / Base / Bad Cases

- Good: sidebar receives a mounted view with project header, namespace epoch, configured sidebar sizing, window rows, agent rows grouped by `agent.window`, and comms action rows using `sender`/`target`.
- Base: before any job starts, an accepted job renders as pending/sending rather than pretending to be running.
- Bad: ccbrd returns only Rust-local fields such as `from_actor`/`to_agent` without Python-compatible `sender`/`target`, causing comms rows to degrade.

### 6. Tests Required

- Daemon: `test_project_view_matches_sidebar_wire_shape` locks the consumer-facing response shape.
- Daemon package: `cargo test -p ccbr-daemon -- --test-threads=1`.
- Sidebar package: `(cd tools/ccb-agent-sidebar && cargo test -- --test-threads=1)`.

### 7. Wrong vs Correct

#### Wrong

```json
{
  "view": {
    "agents": [{"name": "agent1"}],
    "comms": [{"from_actor": "user", "to_agent": "agent1"}]
  },
  "cache": {"ttl_ms": 0}
}
```

#### Correct

```json
{
  "view": {
    "schema_version": 1,
    "project": {"display_name": "repo"},
    "namespace": {"sidebar": {"view": {"comms_limit": 5}}},
    "windows": [{"name": "main", "kind": "agents", "agents": ["agent1"]}],
    "agents": [{"name": "agent1", "window": "main", "activity_state": "pending"}],
    "comms": [{"sender": "user", "target": "agent1", "status_label": "send"}]
  },
  "cache": {"ttl_ms": 1000}
}
```
