# Owner Discovery Thinking Guide

> Purpose: before DDD-sensitive or cross-center work, identify who owns the
> business truth, logic boundary, module boundary, and interface boundary.

This guide captures the N14 owner-discovery method validated against local
planning work, OPC scaffold work, mature external DDD projects, Kaifangqian as
contract-center reference code, and `/home/agnitum/e-contract` as the active
neighbor contract center.

It is a thinking guide, not lifecycle truth. It does not accept owner answers,
open gates, or authorize implementation.

---

## When To Use

Use this before changing or planning anything that touches:

- DDD bounded contexts, aggregates, ontology concepts, or domain events
- cross-center references such as auth, document/archive, BPM, contract,
  settlement, provider, wallet, payment, invoice, tax, legal, AI reuse
- route/DTO/schema fields that look like truth, status, evidence, receipt,
  role, authorization, custody, or admission
- code copied or adapted from local projects, public projects, SDKs, or
  reference implementations
- UI/BFF readback for business state

Do not use it for trivial local UI styling, pure formatting, or one-file
mechanical cleanup with no business meaning.

---

## What Counts As Owner

Owner means the accountable responsibility center for a fact or boundary.

Valid owner axes:

- business upstream/downstream owner
- logic upstream/downstream owner
- module upstream/downstream owner
- interface upstream/downstream owner
- bounded-context / ontology concept owner
- truth / evidence / receipt / admission owner

Not owner:

- original code author
- open-source maintainer
- recent git committer
- package folder name by itself
- CCB/Trellis/CodeGraph/review artifact by itself
- UI, BFF, cache, generated projection, or local read model by itself

Those sources may route questions. They cannot fill owner truth.

---

## First-Principles Split

For every field, endpoint, document, or module, classify exactly one primary
meaning:

| Meaning | Question | Typical result |
|---------|----------|----------------|
| Source fact | Who can change the real business fact? | owning bounded context |
| Candidate | Who must review before this can count? | owner receipt required |
| Readback | Who produced the state being displayed? | read-only projection |
| Receipt | Who signed off the evidence? | reviewed owner answer |
| Command | Who is allowed to cause effects? | command/admission owner |
| Custody | Who stores and preserves the artifact? | document/archive owner |
| Runtime | Who owns the running integration? | provider/runtime owner |
| Legal/effective state | Who can assert effect? | legal/regulated gate owner |

If more than one meaning appears, split the field or route. Do not create a
generic owner that hides the split.

---

## MECE Owner Lanes

Use these lanes as defaults for N14/O13-style platform work:

| Lane | Owns | Does not own |
|------|------|--------------|
| Vertical business app | business action language, local UI workflows, consumer readback | reusable platform truth |
| APAM / process asset | reusable process state, missing evidence, next action labels | document custody, signing runtime, settlement finality |
| Document/archive capability | document evidence assets, manifest/hash, retention, redaction, export rules | contract effective state |
| BPM/workflow capability | workflow step readback and allowed-action projection | platform truth or legal effect |
| Contract center | contract source facts, credentialization review, contract snapshots, signing evidence packs | payment, invoice, tax, ledger, legal judgment |
| Native signing/verification | PDF signing, seal/cert/hash/verification/report capability | production CA legal effect unless gated |
| External provider gateway | SDK call, callback verification, provider status mapping, download, routing | provider business admission and credentials truth |
| Auth/organization center | subject/account/member/org/role/authorization refs | vertical admission or contract role state |
| Settlement center | settlement admission, payment, invoice, tax, ledger, reconciliation, finality | contract source facts |
| AI reuse | read-only reuse, suggestions, retrieval | source truth or write authority |

If a new lane is needed, record it as `owner_required` until a gate accepts it.

---

## Reference Code Intake

Reference code helps find responsibility surfaces, not owner truth.

Use mature projects and open-source code to extract:

- bounded contexts
- aggregate boundaries
- route/service/entity clusters
- upstream/downstream interfaces
- required evidence classes
- no-go surfaces

Never extract:

- owner from author/committer
- owner from repository popularity
- owner from package name without domain corroboration
- production readiness from test/demo code

For Kaifangqian-style contract-center references, extract responsibility
surfaces such as RE/RU, template, document/control, seal, cert, verification,
callback, task, tenant, permission. Then map them to local platform centers.

---

## Required Output Shape

When work is DDD-sensitive, produce or maintain a small table:

| Field / route / module | Meaning | Candidate owner | Evidence | Status | Non-claim |
|------------------------|---------|-----------------|----------|--------|-----------|
| `field_name` | source fact / candidate / readback / receipt / command | `owner_name` or `owner_required` | path or reviewed answer | accepted / amended / blocked / owner_required | no provider/legal/settlement/etc. |

Allowed statuses:

- `accepted` only after reviewed owner receipt or owning gate
- `amended` when owner shape is useful but needs exact authority/non-claim
- `blocked` when the surface is regulated or not open
- `owner_required` when no accountable owner answer exists
- `none_with_reason` when intentionally no owner applies

---

## Before Writing Code

- [ ] Search local code and docs for the concept before inventing a new owner.
- [ ] Check neighboring project/reference code for responsibility surfaces.
- [ ] Separate source fact, candidate, readback, receipt, command, custody,
      runtime, and legal/effective state.
- [ ] Name one accountable owner per field or mark `owner_required`.
- [ ] Add a non-claim for provider, CA, auth/session/security, DB/schema,
      wallet/ledger, payment, settlement, invoice, tax, legal, lifecycle, AI,
      or production truth when nearby.
- [ ] Keep UI/BFF as readback/projection unless an accepted gate says otherwise.
- [ ] If owner is missing, fail closed in code: blocked, unavailable,
      owner_required, candidate_only, readback_only, or not_open.

---

## Wrong Vs Correct

### Wrong

```text
owner = contract_center
```

Why wrong: contract center contains multiple responsibility surfaces.

### Correct

```text
contract_core_source_fact_owner
contract_center_native_signing_verify_owner
contract_center_external_provider_gateway_owner
contract_evidence_handoff_owner
contract_settlement_linkage_owner
```

Each owner maps to a different effect boundary.

### Wrong

```text
signer_ref_owner = auth_center
therefore signer task status belongs to auth_center
```

### Correct

```text
signer identity ref -> auth_center
contract signer role/order/task status -> contract_core_source_fact_owner
```

### Wrong

```text
verified: true
```

### Correct

```text
verified: true
source: reviewed verifier receipt
scope: providerFlowId + signRuId + documentId
non_claim: no real CA/legal effect unless gated
```

---

## Stop Conditions

Stop and route to planning/review when:

- no accountable owner can be named
- two owners both appear to own the same source fact
- a field can imply legal, settlement, payment, invoice, tax, ledger, auth,
  provider-live, CA, or production truth
- reference code suggests behavior but local platform owner is absent
- implementation would make UI/BFF/cache/projection own reusable truth

The lazy fix is usually a smaller field: `*_ref`, `*_receipt_ref`,
`candidate_*`, `readback_*`, or `owner_required`, not a bigger framework.
