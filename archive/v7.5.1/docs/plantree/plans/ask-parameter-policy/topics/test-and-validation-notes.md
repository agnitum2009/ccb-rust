# Test And Validation Notes

Date: 2026-06-07

## Static Tests

Run focused tests from the source checkout:

```bash
python -m pytest test/test_ask_skill_templates.py test/test_v2_ask_service.py
```

Expected coverage:

- inherited shell ask skills still use `command ask`
- inherited ask skills do not contain Chinese text
- inherited ask skills preserve submit-only diagnostics rules
- parser and submit service still map `--callback`, `--artifact-request`,
  `--artifact-reply`, and `--artifact-io` into route options correctly

## External Source Runtime Validation

Run source-under-test validation from an external CCB project, not from
`/home/bfly/yunwei/ccb_source`.

Default project:

```text
/home/bfly/yunwei/test_ccb2
```

Source command:

```text
/home/bfly/yunwei/ccb_source/ccb_test
```

Before checking managed-home skill projection, install the source inherited
skills into the isolated provider source home. `ccb_test` validates source
runtime behavior, but it does not install updated inherited skill assets into a
fresh `CCB_SOURCE_HOME` by itself.

```bash
cd /home/bfly/yunwei/test_ccb2
bash -lc 'set -euo pipefail
export HOME=/home/bfly/yunwei/test_ccb2/source_home
export CODEX_HOME=/home/bfly/yunwei/test_ccb2/source_home/.codex
export CCB_LANG=en
export CCB_SOURCE_KIND=source
export CCB_SOURCE_ROOT=/home/bfly/yunwei/ccb_source
source /home/bfly/yunwei/ccb_source/install.sh
install_codex_skills
install_claude_skills'
```

Suggested worker lane after source-home skill install:

```bash
cd /home/bfly/yunwei/test_ccb2
CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home HOME=/home/bfly/yunwei/test_ccb2/source_home /home/bfly/yunwei/ccb_source/ccb_test doctor
CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home HOME=/home/bfly/yunwei/test_ccb2/source_home /home/bfly/yunwei/ccb_source/ccb_test
CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home HOME=/home/bfly/yunwei/test_ccb2/source_home /home/bfly/yunwei/ccb_source/ccb_test reload --dry-run
```

Validation targets:

- source `ccb_test` refuses to run from the source checkout but works from the
  external project
- startup succeeds or reports a known environment blocker
- projected managed homes include the updated ask skill text for configured
  providers
- Claude inherited skills may be symlink-projected from the source home; use
  `find -L` or inspect the symlink target when validating projected files
- ask submit fast path still accepts at least one artifact request submission
  when a configured agent is available

Do not use installed release `ccb` to validate source changes. Do not run
source runtime commands from the source checkout.

## Real Home Provider Smoke

Use this lane when the goal is to validate source `ccb_test` behavior while
reusing the real logged-in provider state from `/home/bfly`.

```bash
cd /home/bfly/yunwei/test_ccb2
env -u CCB_SOURCE_HOME /home/bfly/yunwei/ccb_source/ccb_test kill
env -u CCB_SOURCE_HOME /home/bfly/yunwei/ccb_source/ccb_test doctor
env -u CCB_SOURCE_HOME /home/bfly/yunwei/ccb_source/ccb_test
```

Before this lane validates the updated skill text, install inherited skills into
the real provider home:

```bash
cd /home/bfly/yunwei/test_ccb2
bash -lc 'set -euo pipefail
export HOME=/home/bfly
unset CCB_SOURCE_HOME
export CODEX_HOME=/home/bfly/.codex
export CCB_LANG=en
export CCB_SOURCE_KIND=source
export CCB_SOURCE_ROOT=/home/bfly/yunwei/ccb_source
source /home/bfly/yunwei/ccb_source/install.sh
install_codex_skills
install_claude_skills'
```

Observed on 2026-06-07:

- `doctor` reported `home: /home/bfly` and mounted healthy after restart.
- Codex managed home received `/home/bfly/.codex/auth.json`.
- Claude managed home received `/home/bfly/.claude/settings.json` env keys,
  including `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_BASE_URL`.
- Codex and Claude managed ask skills contained the then-current legacy
  task-relationship heading, `Artifact flags are orthogonal`, the 4 KiB spill
  fallback rule, and callback-chain guidance.
- Plain skill visibility smoke completed for Codex and Claude.
- `--artifact-request` smoke completed and created an ask-request artifact.
- `--artifact-reply` smoke completed and stored the completion reply artifact.
- `--silence` independent route smoke completed.
- Codex and Claude queues returned to idle; no certificate or login blocker
  reproduced in this lane.

## Automatic Parameter Decision Smoke

Run this lane when validating whether the ask skill causes an agent to choose
reasonable parameters without the caller naming the flags. Submit parent tasks
to `codexer`, let it delegate to `clauder`, then inspect raw CCB records and
artifact directories.

Observed on 2026-06-07 with real `/home/bfly` provider home:

| Scenario | Parent Job | Expected Decision | Observed Decision | Evidence |
| --- | --- | --- | --- | --- |
| Direct answer | `job_4552d628eebd` | no child ask | no child ask | parent replied `DIRECT_BASELINE_OK` |
| Independent delegation | `job_30d2583e1dbe` | `--silence` | `--silence` | child message `msg_10153f7df64a` had `silence_on_success=true` |
| Dependent delegation | `job_38b9381f2b85` | `--callback` | `--callback` | child reply `CHILD_S2_RESULT_OK` reached parent final reply |
| Exact transient JSON input | `job_b4ddc8c18112` | `--callback --artifact-request` | `--callback --artifact-request` | `codexer-to-clauder-art_0de9170ab5524bd2.txt` preserved the JSON block |
| Structured long child report | `job_dc603ec8454e` | `--callback --artifact-reply` | `--callback --artifact-reply` | child completion reply artifact `job_dd3cf16f5d85-art_691e383e09a44c4e.txt` |
| Exact input plus long report | `job_f31a4862a9bd` | `--callback --artifact-io` | `--callback --artifact-io` | request artifact `codexer-to-clauder-art_f34caf4bea2d44c0.txt` and reply artifact `job_c1b6c1e8cd91-art_b5b832b8e8a64890.txt` |

Important observation: ask-request artifacts are best verified from
`.ccb/ccbd/artifacts/text/ask-request/`. The raw message record `payload_ref`
can remain `null` even when the delivered child request was artifact-backed.
Use `reply_artifact` in `replies.jsonl` for artifact-reply verification.

## Real Pressure Test

Observed on 2026-06-08 in `/home/bfly/yunwei/test_ccb2` with real
`/home/bfly` provider home after reinstalling the latest inherited skills and
restarting source `ccb_test`.

Environment checks:

- `doctor` reported `home: /home/bfly`.
- `ccbd_state=mounted`, `ccbd_health=healthy`.
- Managed Codex home received `auth.json`.
- Managed Claude settings received `ANTHROPIC_AUTH_TOKEN` and
  `ANTHROPIC_BASE_URL`.
- Managed Codex and Claude ask skills contained `Decision Card`,
  `Prefer repo paths when the target can read files directly`,
  `Avoid --silence --artifact-reply`, and the output-policy guardrail.

Codex-parent pressure group submitted six queued parent jobs to `codexer`:

| Scenario | Parent Job | Observed Result |
| --- | --- | --- |
| Direct answer | `job_22afb3177cf4` | replied `ASK_STRESS_C0_DIRECT_OK`, no child ask |
| Independent delegation | `job_bebc06ce1c12` | selected `--silence`; child message `msg_8e2994e516e1` had `silence_on_success=true` |
| Dependent delegation | `job_0802e88f72ca` | selected `--callback`; parent received `ASK_STRESS_C2_CHILD_OK` |
| Exact JSON input | `job_412de1c6d982` | selected `--callback --artifact-request`; request artifact `codexer-to-clauder-art_1e345280de83411e.txt` |
| Long child report | `job_40890e657b91` | selected `--callback --artifact-reply`; child reply artifact `job_6fa5edd440cc-art_193f154be5264c92.txt` |
| Exact input plus long report | `job_6c272148bc92` | selected `--callback --artifact-io`; request artifact `codexer-to-clauder-art_9a9fef02cf814495.txt`, reply artifact `job_474950d59d1d-art_d8f98fe685a749e0.txt` |

Claude-parent reverse pressure group submitted four queued parent jobs to
`clauder`:

| Scenario | Parent Job | Observed Result |
| --- | --- | --- |
| Direct answer | `job_642d726e8af1` | replied `ASK_STRESS_R0_DIRECT_OK`, no child ask |
| Independent delegation | `job_522180f8bd41` | selected `--silence`; child message `msg_78d58e03ed16` had `silence_on_success=true` |
| Dependent delegation | `job_d05240daf518` | selected `--callback`; parent received `ASK_STRESS_R2_CHILD_OK` |
| Exact input plus long report | `job_d2b5b2caebf0` | selected `--callback --artifact-io`; request artifact `clauder-to-codexer-art_d4cbd2dba5044820.txt`, reply artifact `job_f01c0b5e2d10-art_ea042060d374470e.txt` |

Final state:

- `codexer` queue depth 0, runtime idle, health restored.
- `clauder` queue depth 0, runtime idle, health restored.
- No login or certificate blocker reproduced.

## Result Intent Targeted Smoke

Observed on 2026-06-08 after updating the Decision Card from relationship-first
to result-intent-first and reinstalling inherited skills into the real provider
home.

Validated managed skill projection in `/home/bfly/yunwei/test_ccb2`:

- `Result intent`
- `--silence`: publish/execute task; success result not needed
- `--compact`: result wanted, but only distilled status/findings
- `+ --artifact-reply`: consultation/analysis/report where full text should be
  preserved
- plain `ask`: short question or short handoff

Targeted parent jobs to `codexer`:

| Scenario | Parent Job | Observed Result |
| --- | --- | --- |
| Publish-only task | `job_3d30cb5f7514` | selected `--silence`; child message `msg_94323742fc98` had `silence_on_success=true` |
| Short result wanted | `job_719ff27843be` | selected `--callback --compact`; parent received `RESULT_INTENT_COMPACT_CHILD_OK` |
| Full consultation analysis | `job_c04197619749` | selected `--callback --artifact-reply`; child message `msg_6b1fd86d5079` had `reply_artifact=true`, artifact `job_d099d7ac440f-art_a2726319eb1b407a.txt` |

Final state:

- `codexer` queue depth 0, runtime idle, health restored.
- `clauder` queue depth 0, runtime idle, health restored.
