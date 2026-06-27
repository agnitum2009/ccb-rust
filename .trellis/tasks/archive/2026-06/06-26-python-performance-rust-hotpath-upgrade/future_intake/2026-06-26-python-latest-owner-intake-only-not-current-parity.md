# Rust/Python Owner Alignment Risk List — 2026-06-26

> Scope correction 2026-06-27: active Rust/ccbr parity target is Python `v7.5.2`. The Python-latest items in this file are future source-intake risks only, not current parity blockers.

## Owner-method binding

- method source: `/mnt/g/owner/owner-method-kit/README.md`
- target project root: `/home/agnitum/ccb`
- Python 7.5.2 parity evidence root: `/home/agnitum/ccb-git` tag `v7.5.2`
- Python latest evidence root: `/home/agnitum/ccb-git` (`v7.6.x` branch evidence), for future intake risk only
- Rust main evidence root: `/home/agnitum/ccb` (`python-rust/rolepacks-versioning-translation`)
- legacy proof line: `/home/agnitum/ccb/ccb-legacy`
- record type: owner-risk inventory, not confirmed owner records
- forbidden write zone: `/mnt/g/owner/owner-method-kit/**`
- local authority checked: AGENTS/Trellis task `06-26-python-performance-rust-hotpath-upgrade`

## Method interpretation used

The kit says source snapshots are evidence only, not owner truth. Therefore this audit separates:

```text
Python latest source evidence != accepted owner truth
Rust implementation surface != accountable owner
candidate/risk != confirmed_owner
```

A missing Rust implementation is recorded only when it blocks a consequence-bearing closure path across an owner surface.

## Confirmed evidence summary for future latest-intake risk

The active 7.5.2 parity inventory lives in `2026-06-27-rust-functional-parity-to-python-7.5.2.md`. The bullets below intentionally describe later Python-source drift and must not be read as current 7.5.2 blockers.

## Confirmed evidence summary

- Python latest changed substantially after `v7.5.2`: `git diff --name-status v7.5.2..HEAD` shows new runtime performance helpers, `provider_backends/zai`, mobile gateway files, callback-continuation safety docs, runtime accelerator docs/crate, and rolepack-system updates.
- Python daemon handlers: 27 registered handlers; Rust daemon handlers: 33 registered handlers.
- Python-only daemon handler: `project_sidebar_click`.
- Rust-only daemon handlers: `ask`, `cleanup`, `fault_arm`, `fault_clear`, `fault_list`, `logs`, `maintenance_tick`.
- Python provider backend dirs: 17 providers including `zai`.
- Rust provider backend dirs: 16 providers; no `zai` provider directory found.
- Python latest includes mobile gateway source/docs/tests; Rust main grep found no `mobile`, `cloudflare`, `tailnet`, or `relay` implementation under `rust/crates` / `tools`.
- Python latest and ccb-legacy have `ccb-runtime-accelerator`; Rust main workspace does not include that crate, by current design.

## Future-intake owner risks, not active 7.5.2 blockers

| Record | Surface | Lifecycle stage | Gate failure | Owner risk | Next owner action |
| --- | --- | --- | --- | --- | --- |
| `provider:zai` | capability/interface | legacy_upgrade | Python latest has `lib/provider_backends/zai`; Rust main has no `zai` provider surface | Rust main cannot claim provider parity with Python latest for Z.ai launch/session/readback until owner decides whether Z.ai is in-scope for ccbr intake | Create a `zai` provider intake owner demand: provider truth, CLI launch contract, session binding, completion/readback evidence, tests |
| `mobile_gateway` | capability/interface/lifecycle_gate | legacy_upgrade | Python latest adds `mobile_gateway` and `ccb mobile`; Rust main has no matching implementation | Mobile/Cloudflare/Tailnet access paths have no Rust owner path; shipping Rust as latest-compatible would silently drop remote/mobile control capability | Decide whether mobile gateway is out-of-scope for ccbr, Python-only, or Rust intake; if intake, freeze public CLI/API and security boundary first |
| `project_sidebar_click` | interface/projection | legacy_upgrade | Python daemon exposes `project_sidebar_click`; Rust daemon has sidebar focus ops but no same handler name | Sidebar click semantics may be split between tool/CLI and daemon; without an owner receipt this can regress UI click behavior even if focus works elsewhere | Map sidebar click event flow end-to-end and either add compatible daemon op or record explicit divergence with tests |
| `runtime_accelerator_intake` | capability/performance/lifecycle_gate | active_development | Python latest + ccb-legacy contain `ccb-runtime-accelerator`; Rust main lacks crate because ccbr owns a different `.ccbr` daemon architecture | Not a direct parity bug, but a selective-intake decision is still open: proven accelerator code may be shared or intentionally left Python-only | After ccb-legacy proof, decide shared crate vs no-intake. Do not import `.ccb` assumptions into ccbr |
| `provider_catalog_vs_execution_registry` | relationship/capability | legacy_upgrade | Rust core registry and provider execution registry are not the same owner; core comments preserve API shape while adapters live elsewhere | A provider can appear in catalog-ish code without executable launch/readback support, causing false parity claims | Maintain two gates per provider: manifest/catalog gate and execution/session/readback gate |
| `rolepack_current_store` | policy/lifecycle_gate | legacy_upgrade | Python latest changed rolepack current-store/restart adoption docs/code; Rust has large rolepack implementation, but no receipt tying latest Python decision 007 to Rust behavior | Rust may be feature-rich but still not proven aligned to the latest accepted Python rolepack lifecycle decision | Compare Python decision `007-single-current-store-and-restart-adoption` against Rust rolepack current pointer/lock behavior; add a targeted parity receipt/test |
| `callback_continuation_safety` | evidence_admission/lifecycle_gate | legacy_upgrade | Python latest adds callback-continuation safety plan; Rust ask/dispatcher paths exist but need owner-level evidence against that newer safety contract | Callback chain/reply delivery may pass old 7.5.2 parity but miss newer continuation identity/safety constraints | Freeze Python latest callback safety scenarios and run/port Rust tests before claiming latest-compatible ask semantics |

## Promotion decision

```text
promote_with_explicit_owner_risk
```

Reason: Rust main remains a 7.5.2-aligned line. The risks above only block a future Python-latest compatibility claim; they do not block the current 7.5.2 parity work.

## Non-claims

- This audit does not confirm business/product owners.
- This audit does not say every Python latest addition must enter ccbr.
- This audit does not merge `ccb-legacy` and `ccbr`.
- This audit does not disable or recommend disabling Codex hooks.
