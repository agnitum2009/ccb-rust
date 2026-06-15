# Decision 001: Source Ownership, Not Text Deduplication

Date: 2026-06-07

## Status

Accepted for planning.

## Context

Managed provider memory can duplicate content when CCB bundles sources that the
provider also loads natively, or when old CCB install blocks remain in provider
user memory. A tempting fix is to deduplicate rendered text by exact, fuzzy, or
semantic similarity.

That approach is brittle:

- User-authored rules may intentionally resemble CCB-generated rules.
- Similar wording can carry different scope or authority depending on its
  source.
- Fuzzy or semantic removal is hard to explain in diagnostics.
- Provider-native loading behavior differs across Claude, Codex, OpenCode, and
  Gemini.

## Decision

CCB memory projection must be governed by a provider memory source ownership
manifest:

```text
source kind -> native loaded? -> CCB bundle included? -> filter policy
```

Filtering is allowed only when it is source-aware and policy-backed, such as
stripping CCB install blocks from inherited provider user memory. The renderer
must not use fuzzy text similarity to decide whether to remove user memory.

## Consequences

- Inclusion decisions become provider policy, not renderer heuristics.
- Diagnostics can explain why each source was included, excluded, or filtered.
- Claude, Codex, and OpenCode can differ where their native memory behavior
  differs, while sharing source-kind vocabulary.
- Tests should assert source policy and rendered section counts rather than
  asserting accidental text-size reductions.

