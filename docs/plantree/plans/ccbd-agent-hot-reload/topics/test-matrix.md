# Test Matrix

Date: 2026-05-29

## Automated Unit Tests

- Reload no-op:
  - old and new config identities match;
  - no namespace mutation;
  - no runtime authority writes;
  - project view cache is invalidated only if needed.
- Reload dry-run:
  - diff classes are returned;
  - no tmux commands are issued;
  - no graph is published;
  - no lifecycle or lease signature changes.
- Handler graph routing:
  - after graph replacement, `submit`, `project_view`, `ping`, and focus
    handlers resolve the new graph;
  - old graph retention is bounded after in-flight requests finish.
- Invalid config:
  - parse/validation error returned;
  - old `app.config_identity` remains published;
  - keeper-compatible signature remains old;
  - no tmux calls.
- Add agent to existing window:
  - old agent pane ids unchanged;
  - new agent pane created with correct CCB tmux identity options;
  - registry knows the new agent;
  - supervision sees the new desired agent.
- Add new window:
  - existing window and pane ids unchanged;
  - new tmux window exists;
  - new sidebar pane exists when sidebar mode is `every_window`;
  - new agents in that window are mounted.
- Existing agent provider/workspace/model/key/url change:
  - classified as `unsafe_requires_restart` while runtime is running;
  - existing pane and runtime record are untouched.
- Existing agent removed from `[windows]`:
  - dry-run reports `unload_agent`;
  - idle unload retires runtime and removes only the target pane;
  - busy unload enters bounded draining or returns a stable rejection;
  - existing unrelated processes are not killed by reload.
- Existing agent provider/workspace/model/key/url change after replacement is
  enabled:
  - idle replace advances runtime authority epoch;
  - busy replace enters bounded `pending_replace`;
  - provider session continuity is not claimed without provider-specific proof.
- Existing agent moved to another window:
  - rejected as layout/ownership move;
  - existing pane remains in place.
- Busy agent preservation:
  - fake runtime reports `BUSY`;
  - additive reload succeeds for unrelated new agent;
  - busy runtime authority is unchanged.
- Keeper signature continuity:
  - successful reload updates daemon ping payload signature;
  - keeper `daemon_matches_project_config()` returns true after reload.
- Project view/sidebar:
  - successful reload invalidates cache;
  - next `project_view` includes new agents/windows;
  - sidebar refresh signal is sent to managed sidebars.
- Performance gates:
  - no-op dry-run does not increase steady-state heartbeat work;
  - project-view cache hits remain cache hits;
  - handler graph read path does not use a contended global mutex.

## Integration Tests With Fake Tmux

- Add one agent to an existing two-agent window and assert only one `split-pane`
  is issued.
- Add a new window and assert existing windows receive no `kill-window`,
  `respawn-pane`, or recreation calls.
- Sidebar-enabled topology creates exactly one sidebar pane for each new window.
- Failure after validation but before publish leaves old bundle active.
- Failure during namespace patch leaves old config active and records a
  recoverable reload failure.
- Old service graph is retained only for in-flight requests and then released.
- Drain timeout and pending queue bounds prevent unbounded pending reload state.

## Manual `test_ccb2` Tests

- Start a project with two windows and four agents.
- Start a long-running/manual task in `agent2`.
- Edit `.ccb/ccb.config` to add `agent5` to an existing window.
- Run `ccb reload`.
- Verify via tmux screenshot:
  - `agent2` remains in the same pane and continues running;
  - `agent5` appears in a new managed pane;
  - sidebar shows `agent5`;
  - no global refresh/restart occurred.
- Repeat by adding a new window with one new agent.
- Try changing `agent2` provider/workspace/model while it is running; reload
  must refuse without killing the pane.
- Try deleting a running agent; reload must refuse or mark pending removal
  without killing the pane.
- Run `ccb reload --dry-run` before each mutating manual test and verify it
  reports the same planned operation that the mutating command later executes.
- Measure idle/sidebar-open CPU and RSS before and after the reload feature is
  installed.

## Release Gate

Hot reload is releasable only when:

- accepted additive reload preserves old pane ids in automated and manual
  tests;
- busy existing agents continue running through reload;
- unsafe diffs are rejected without side effects;
- keeper does not restart after successful reload;
- project view/sidebar reflect the new config immediately after reload;
- `ccb kill` and normal cold start behavior remain unchanged.
- steady-state CPU/RSS does not grow continuously after repeated dry-run and
  accepted reload operations;
- draining and pending replacement have tested timeout and bound behavior.
