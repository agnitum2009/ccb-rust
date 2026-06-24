# ccbr-daemon

Rust implementation of the CCBR daemon control plane, migrated from `lib/ccbrd/`
and `lib/fault_injection/`.

## Scope of this pass

This crate provides a working daemon foundation focused on the critical paths
for `ccbr start`, `ccbr stop`, `ccbr status`, and provider pane management:

- Unix-domain-socket RPC server (`socket_server`)
- Project namespace persistence (`services::project_namespace`)
- Start/stop flows with pluggable tmux backend (`start_flow`, `stop_flow`)
- Job dispatcher and RPC handler registry (`services::dispatcher`, `handlers`)
- Health monitoring (`services::health`)
- Supervision loop and backoff (`supervision`)
- Fault injection service for tests (`fault_injection`)

## RPC protocol compatibility

The daemon accepts both the legacy Python protocol (`op` + `request`) and the
Rust CLI protocol (`method` + `params`). Responses include a flattened payload
for Python clients and a `result` field for CLI clients.

## Stubs / known limitations

The following advanced features are intentionally stubbed or simplified in this
migration pass and are documented here for follow-up work:

- **Full service graph publishing**: Python `app_runtime.service_graph` is
  replaced with direct service ownership on `CcbdApp`.
- **Reload transactions**: `reload/` modules contain only plan/transaction
  skeletons; live config reload is not implemented.
- **Project focus / project view state stores**: handlers return basic data from
  the registry and namespace only.
- **Completion tracking / message bureau integration**: dispatcher tracks jobs
  but does not yet integrate with `ccbr-completion` or `ccbr-mailbox` for actual
  provider execution.
- **Keeper process integration**: `CCBR_KEEPER_PID` and keeper lifecycle are not
  wired.
- **Ownership guard lease persistence**: ownership records are in-memory only
  in this pass.
- **Runtime adoption / binding generations**: runtime authority adoption from
  prior daemon generations is not implemented.
- **Tmux layout integration**: start flow uses direct tmux session creation
  rather than the full `ccbr-terminal` auto-layout backend.

## Testing

```bash
cd /home/agnitum/ccbr/rust
cargo test -p ccbr-daemon
cargo clippy -p ccbr-daemon
```
