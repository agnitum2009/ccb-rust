# ccb.archi

`ccb.archi` is an architecture reviewer Role Pack backed by
[Architec](https://github.com/SeemSeam/architec), Hippo, and llmgateway.

The role's mission is to protect maintainability by reviewing architecture
drift, duplicated logic, shadow implementations, boundary pressure, topology
pressure, stale compatibility code, and high-risk hotspots.

## Typical CCB Binding

```toml
version = 2

[windows]
main = "agent1:codex, ccb.archi:codex"
```

At runtime CCB resolves `ccb.archi:codex` to the project-local agent `archi`
and keeps the stable role id in the generated agent overlay.

## Commands

```bash
ccb roles install ccb.archi
ccb roles doctor ccb.archi
ccb roles update ccb.archi
ccb roles add ccb.archi:codex
ccb reload
```

`ccb roles install` and `ccb roles update` install Role Pack assets into the
system role store and prepare the CCB-owned Architec venv plus `ccb-archi`
wrapper by default.

## Tooling

The role prefers `ccb-archi`, a wrapper installed by CCB into a CCB-owned venv.
If unavailable, skills fall back to `archi`.

Architec uses llmgateway for LLM credentials and model routing. Do not store
llmgateway secrets in `.ccb/ccb.config`.

## Generated Outputs

Architec writes advisory output under `.architec/`; Hippo writes structural
snapshots under `.hippocampus/`.

Start with `.architec/architec-summary.md`, then use
`.architec/architec-analysis.json` for exact scores and concern data.
