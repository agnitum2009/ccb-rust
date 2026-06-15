# CCB Self Skill Draft Review And Test Evidence

Date: 2026-06-09

## Draft Paths

Production target, after review: `/home/bfly/yunwei/agent-roles-spec/roles/ccb-self/`
or the equivalent accepted catalog path for `agentroles.ccb_self`.

Current reviewable draft in this CCB plan:

- [drafts/agentroles.ccb_self/role.toml](../drafts/agentroles.ccb_self/role.toml)
- [drafts/agentroles.ccb_self/memory.md](../drafts/agentroles.ccb_self/memory.md)
- [ccb-self-diagnose](../drafts/agentroles.ccb_self/skills/codex/ccb-self-diagnose/SKILL.md)
- [ccb-self-recover](../drafts/agentroles.ccb_self/skills/codex/ccb-self-recover/SKILL.md)
- [ccb-self-chain](../drafts/agentroles.ccb_self/skills/codex/ccb-self-chain/SKILL.md)
- [ccb-config](../drafts/agentroles.ccb_self/skills/codex/ccb-config/SKILL.md)
- [config-contracts.md](../drafts/agentroles.ccb_self/skills/codex/ccb-config/references/config-contracts.md)
- [runtime-authority.md](../drafts/agentroles.ccb_self/references/runtime-authority.md)
- [recovery-runbooks.md](../drafts/agentroles.ccb_self/references/recovery-runbooks.md)
- [tmux-ccb-quickstart.md](../drafts/agentroles.ccb_self/references/tmux-ccb-quickstart.md)
- [tools/doctor.py](../drafts/agentroles.ccb_self/tools/doctor.py)

## Core Skill Content

`ccb-self-diagnose`:

- Read-only triage for daemon graph, tmux namespace/panes, provider context,
  queue/inbox/trace, replies/artifacts, config drift, and storage boundaries.
- Requires authority/evidence/residue separation.
- Uses CCB control-plane diagnostics and read-only tmux evidence first.
- Refuses mutation and reports a concise status/domain/authority/evidence/
  residue/next-action result.

`ccb-self-recover`:

- Runtime recovery after diagnosis.
- Enforces busy/pending checks before `clear` or `restart`.
- Provider/API recovery flow is exactly edit config -> `ccb config validate` ->
  `ccb reload --dry-run` -> `ccb reload` -> re-check affected agents -> guarded
  per-agent restart only when supported and still needed.
- Treats `reload` as config materialization, not provider process refresh.
- Refuses restart-all and raw tmux mutation.

`ccb-self-chain`:

- Traces ask/job/message/reply/artifact/callback lineage.
- Reads full artifact files before acting; preview text is not enough.
- Distinguishes `repair retry`, `repair resubmit`, and `repair ack`.
- Does not use pane restart as the first repair for lineage issues.

`ccb-config`:

- Private built-in `agentroles.ccb_self` skill, not a global inherited skill.
- Owns `.ccb/ccb.config` design/edit/validate/reload readiness.
- Runs `ccb config validate` after every edit and `ccb reload --dry-run` before
  reload.
- May run `ccb reload` only after gates and materialization intent.
- Outputs affected agents for `ccb-self-recover`; it does not run restart,
  kill, clear, repair, or raw runtime writes.

## Review Status

| Reviewer | Scope | Status | Handling |
| --- | --- | --- | --- |
| coworker | All four skill drafts plus test plan | Completed; artifact `/home/bfly/yunwei/ccb_source/.ccb/ccbd/artifacts/text/completion-reply/job_d8a3fa6086ee-art_3ee15861074c44cc.txt` | Processed below |
| reviewer3 | All four skill drafts plus test plan | Completed; artifact `/home/bfly/yunwei/ccb_source/.ccb/ccbd/artifacts/text/completion-reply/job_7b7db3db986f-art_026de24551dd4813.txt` | Processed below |

After both reviews were complete and processed, reviewer contexts were cleared:

```text
clear_status: ok
cleared_count: 2
skipped_count: 0
failed_count: 0
clear_agent: agent=coworker status=cleared pane_id=%15
clear_agent: agent=reviewer3 status=cleared pane_id=%11
```

Review requests must ask each reviewer to check:

- whether authority/evidence/residue boundaries are enforceable;
- whether recover/config split prevents hidden restart or raw tmux mutation;
- whether chain repair semantics avoid confusing retry/resubmit/ack;
- whether artifact-backed replies require full-file reads before action;
- whether tests prove fast CCB/tmux/agent failure resolution or correctly mark
  current blockers.

## Coworker Review Handling

Accepted and fixed:

- D2: removed source-checkout hardcoded `/home/bfly/yunwei/ccb_source` and
  `/home/bfly/yunwei/test_ccb2` paths from distributable
  `ccb-self-diagnose/SKILL.md`.
- D3/D5: added `ccb ping ccbd` and `ccb doctor logs <agent>` to diagnose.
- D4/R1: added `ccb fault list` diagnostic evidence and guarded
  `ccb fault clear <rule_id|all>` handling for known test residue.
- R2/R3/R4/R5: clarified the then-missing restart fallback, linked `ccb clear`
  to pre-mutation gates, gated `ccb roles update agentroles.ccb_self`, and
  kept restart behind the documented command contract.
- C1/C2/C4: added `project_shutdown` handling, `ccb cancel <job_id>` before
  resubmit/retry for in-flight jobs, and post-repair success checks.
- CF1/CF4/CF5: added config-validate failure handling, explicit `[windows]`
  topology preference, and a projection-boundary note for the private
  same-name `ccb-config` skill.
- X1: replaced placeholder `tools/doctor.py` with a real read-only JSON helper
  that runs `ping ccbd`, `doctor`, `ps`, `queue --detail all`, `fault list`,
  `config validate`, and `reload --dry-run`.

Rejected or narrowed with evidence:

- D1: rejected as a blocker. `ccb queue --detail all` was already validated in
  `/home/bfly/yunwei/test_ccb2` and remains the correct syntax.
- C3: rejected as a blocker. Parser and CLI help confirm
  `ccb repair retry <job_id|attempt_id>`,
  `ccb repair resubmit <message_id>`, and
  `ccb repair ack <agent_name> [inbound_event_id]`.
- CF2/CF3: rejected for this draft. The private `ccb-config/SKILL.md` already
  uses only `ccb config validate`; there is no inline
  `from agents.config_loader import load_project_config` validation block in
  the drafted role skill.
- CF5 name-change suggestion: rejected for now because
  [decisions/002-built-in-ccb-config-skill.md](../decisions/002-built-in-ccb-config-skill.md)
  explicitly keeps the canonical name `ccb-config`. The accepted mitigation is
  to require role projection isolation from inherited/global same-name skills.

Accepted follow-up risk:

- X2/X3/X4 remain follow-up work: add command syntax smoke tests, structured
  MCP/control-plane wrappers, and handoff matrix scenarios before v1 release.

## Reviewer3 Review Handling

Accepted and fixed:

- CR1: added the missing `ccb cancel <job_id>` failure branch to
  `ccb-self-chain/SKILL.md`. If cancel fails or reports a blocking state, the
  skill now stops and does not retry, resubmit, or create a concurrent path.
- RC1: clarified `project_shutdown` handling. The default is now to prefer
  `repair resubmit <message_id>` for fresh work; `repair retry` is allowed only
  when trace proves the job completed before shutdown and the reply is intact.
- RD2: added a read-only provider session/pid-file path and mtime inspection
  step without reading provider-state contents.
- RR2: tightened `fault clear` handling. Recent rules or rules that look like
  active drills require confirmation unless the user explicitly asked to clear
  fault-injection rules.
- CG1: added an explicit dated `.ccb/ccb.config` pre-edit backup step and
  restricted rollback to the backup created for the current edit.

Validated after review:

- RD1: `ccb_test doctor logs codexer` returned `logs_status: ok`, proving the
  `ccb doctor logs <agent>` form works in the isolated test project.
- RR1: `ccb_test fault clear all` returned `fault_status: cleared` with
  `cleared_count: 0`, proving the `ccb fault clear <rule_id|all>` form works
  without active rules. A follow-up `ccb_test fault list` still reported
  `rule_count: 0`.

Accepted follow-up risks:

- CG2 remains a release-blocking implementation requirement outside skill text:
  role projection must isolate the private same-name `ccb-config` skill from
  inherited/global same-name skills before materialization.
- `ccb restart <agent>` and catalog installation remain contract/platform
  blockers, not skill quality blockers.

## Test Environment

All source-runtime validation was run from `/home/bfly/yunwei/test_ccb2`, not
from `ccb_source`:

```bash
HOME=/home/bfly/yunwei/test_ccb2/source_home \
CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home \
/home/bfly/yunwei/ccb_source/ccb_test ...
```

The wrapper diagnosis reported:

- `source_checkout_cwd: no`
- `project_inside_source: no`
- `allowed_source_test_project: yes`
- `effective_roots: /home/bfly/yunwei/test_ccb2`

## Shared Validation Evidence

Main-agent spot check after worker3 handoff:

```bash
HOME=/home/bfly/yunwei/test_ccb2/source_home \
CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home \
/home/bfly/yunwei/ccb_source/ccb_test --diagnose
```

Result summary:

```text
cwd: /home/bfly/yunwei/test_ccb2
source_checkout_cwd: no
project_inside_source: no
allowed_source_test_project: yes
```

Additional spot checks:

- `ccb_test doctor logs codexer`: `logs_status: ok`,
  `runtime_ref: tmux:%1`, `log_count: 4`.
- `ccb_test fault list`: `fault_status: ok`, `rule_count: 0`.
- `CCB_BIN=/home/bfly/yunwei/ccb_source/ccb_test python3
  drafts/agentroles.ccb_self/tools/doctor.py`: JSON `status: ok`,
  `evidence_count: 7`, `failed_count: 0`.

Static draft package check:

```bash
python3 - <<'PY'
...
PY
```

Result:

```text
draft_static_check: ok role=agentroles.ccb_self skills=4 tools=1
```

Draft doctor helper check:

```bash
python3 docs/plantree/plans/ccb-self-role/drafts/agentroles.ccb_self/tools/doctor.py
```

Result summary after coworker fixes:

```text
doctor_status: ok
evidence_count: 7
failed_count: 0
```

The helper emitted valid JSON and ran only read-only CCB control-plane commands.

`ccb config validate` in isolated project:

```text
config_status: valid
config_source_kind: project_config
agents: clauder, codexer, geminier, opencoder
```

`ccb reload --dry-run` in isolated project:

```text
reload_status: ok
dry_run: true
mutation_enabled: false
plan_class: no_change
safe_to_apply: false
future_safe_to_apply: true
```

`ccb doctor` and `ccb ps` in isolated project:

- daemon state mounted and healthy;
- generation `11`;
- four configured agents from the live graph: `clauder`, `codexer`,
  `geminier`, `opencoder`;
- each agent bound to a CCB tmux pane (`%1`, `%2`, `%3`, `%4`);
- `opencoder` reports provider resume unsupported, which is runtime evidence,
  not a restart target authority change.

`ccb queue --detail all` and `ccb pend --inbox --detail codexer`:

- observer surfaces report supplementary snapshots;
- all agents idle with queue depth `0`;
- codexer inbox idle with pending reply count `0`.

`ccb fault list`:

```text
fault_status: ok
rule_count: 0
fault_rule: <none>
```

`ccb fault clear all` in the isolated project:

```text
fault_status: cleared
target: all
cleared_count: 0
```

Follow-up `ccb fault list` still reported `rule_count: 0`.

`ccb ping ccbd`:

- mounted daemon is healthy;
- known agents are `clauder`, `codexer`, `geminier`, `opencoder`;
- namespace and socket evidence are reported by the control plane.

`ccb doctor logs codexer`:

```text
logs_status: ok
agent_name: codexer
provider: codex
runtime_ref: tmux:%1
log_count: 4
```

The command shape is validated. Log contents can include provider UI text, so
skills should summarize relevant non-secret lines rather than copy full logs by
default.

`ccb roles show agentroles.ccb_self`:

```text
roles_status: failed
error: unknown role: agentroles.ccb_self
```

Interpretation: role catalog/package validation is blocked until the draft is
materialized into `agent-roles-spec` or a local editable role source.

`ccb restart codexer`:

```text
restart_status: ok
agent_name: codexer
restartable_agents: codexer, clauder, opencoder, geminier
restart_busy_gate: passed=true runtime_state=idle runtime_queue_depth=0 queue_depth=0 pending_reply_count=0 active_job_id=None active_inbound_event_id=None pending_callback_count=0
restart_result: agent=codexer status=restarted pane_id=%1
```

Interpretation: `ccb restart <agent>` is implemented in the source CLI surface
and uses mounted daemon graph authority. It reports available restart targets,
busy gate evidence, old/new runtime evidence, and refuses unknown targets with
the current restartable agent list.

## Per-Skill Scenarios

### `ccb-self-diagnose`

1. Mounted daemon and pane evidence:
   - Commands: `ccb_test doctor`, `ccb_test ps`.
   - Result: daemon healthy and mounted; live graph has four agents; each has
     tmux pane evidence. This proves the skill can quickly separate live graph
     authority from pane evidence.
2. Queue/inbox evidence:
   - Commands: `ccb_test queue --detail all`,
     `ccb_test pend --inbox --detail codexer`.
   - Result: observer surfaces are explicitly supplementary; queue and inbox
     are idle. This proves the skill should not overstate observer authority.
3. Config drift check:
   - Commands: `ccb_test config validate`, `ccb_test reload --dry-run`.
   - Result: project config valid and reload plan is `no_change`. This proves
     the diagnose skill can route config drift to `ccb-config` without mutation.

### `ccb-self-recover`

1. Provider/config recovery gate:
   - Commands: `ccb_test config validate`, `ccb_test reload --dry-run`.
   - Result: gates run cleanly and non-mutating dry-run reports no change.
   - Expected skill behavior: only after a real edit plus user materialization
     intent may `ccb-config` run `reload`; recover then rechecks affected agents.
2. Guarded restart contract:
   - Command: `ccb_test restart codexer`.
   - Result: `restart_status: ok`; busy gate passed for an idle current-graph
     agent and old/new runtime evidence was reported.
   - Expected skill behavior: use `ccb restart <agent>` only after
     busy/pending checks pass; report blockers instead of using raw tmux
     mutation when the command returns `blocked` or `failed`.
3. Busy/pending precheck:
   - Commands: `ccb_test ps`, `ccb_test queue --detail all`,
     `ccb_test pend --inbox --detail codexer`.
   - Result: target agents idle and queue depth zero.
   - Expected skill behavior: prechecks are available, but no clear/restart was
     executed because this draft validation did not intentionally mutate
     provider contexts.

### `ccb-self-chain`

1. Incomplete job retry candidate:
   - Command: `ccb_test trace job_d5c0c10467fc`.
   - Result: message and attempt state `incomplete`, reply terminal
     `incomplete`, reason `project_shutdown`.
   - Expected skill behavior: consider `repair retry <job_id|attempt_id>` from
     lineage evidence, not pane restart first.
2. Artifact-backed reply with missing full file:
   - Command: `ccb_test trace job_6fa5edd440cc`.
   - Result: reply preview says the completion reply was stored as an artifact
     and must be read from full text path; artifact storage directory currently
     has no files.
   - Expected skill behavior: do not act from preview alone; report artifact
     full-text blocker.
3. Repair command semantics:
   - Commands: `ccb_test repair retry --help`,
     `ccb_test repair resubmit --help`, `ccb_test repair ack --help`.
   - Result: CLI exposes separate `retry <job_id|attempt_id>`,
     `resubmit <message_id>`, and `ack <agent_name> [inbound_event_id]`.
   - Expected skill behavior: choose one based on trace state and user
     maintenance intent.

### `ccb-config`

1. Validation gate:
   - Command: `ccb_test config validate`.
   - Result: project `.ccb/ccb.config` loaded as `project_config` and is valid.
   - Expected skill behavior: every edit must run this before reload discussion.
2. Reload dry-run gate:
   - Command: `ccb_test reload --dry-run`.
   - Result: no-mutation plan produced with `plan_class: no_change`.
   - Expected skill behavior: reload never runs before this dry-run.
3. Role package readiness:
   - Command: `ccb_test roles show agentroles.ccb_self`.
   - Result: `unknown role`.
   - Expected skill behavior: give `ccb roles install agentroles.ccb_self`
     guidance after catalog materialization; do not copy role assets into
     `.ccb` manually.

## Needed Helpers And Tests

Needed for v1:

- Harden production `tools/doctor.py` schema and add targeted optional
  `trace`/`pend`/agent-log collection parameters while keeping default behavior
  read-only and non-secret.
- Read-only MCP or helper surfaces for runtime snapshot, agent status, lineage,
  queue/inbox, reload plan, storage summary, tmux namespace, pane list, pane
  text capture, and pane activity sampling.
- Catalog fixture for `agentroles.ccb_self` in the role package test suite.
- Role projection tests proving only `ccb_self` receives the full private
  `ccb-config` skill.
- Contract tests for guarded one-agent restart, unknown targets, busy/pending
  blockers, and no restart-all/force/window restart surface.
- Artifact lifecycle fixture proving the chain skill reads full artifact text
  before acting and reports a blocker when the file is gone.

Deferred:

- Screenshot/OCR MCP tools and visual inspection fixtures.
- Mutating MCP wrappers for reload, clear, repair, and restart after the
  corresponding CLI commands and gates are stable.
