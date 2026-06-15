# Developer Manual Outline

Date: 2026-06-10

## Proposed Title

`CCB Developer Manual: Architecture, Runtime, and Communication Internals`

## Proposed Structure

1. Preface
   - audience, evidence model, notation, repository assumptions;
   - how to read source references and generated evidence.
2. CCB Concepts
   - project anchor, agent, provider, role, pane, window, backend, mailbox,
     job, request, reply, artifact, memory bundle.
3. Source Tree Map
   - module map from `lib/`, `config/`, `tools/`, tests, and generated runtime
     boundaries;
   - Archi/Hippo code-map summary and hotspot warnings.
4. Startup And Supervision
   - CLI entry, project discovery, keeper, ccbd, lifecycle, lease, socket,
     namespace materialization, pane supervision, kill/shutdown.
5. Configuration And Topology
   - config loading precedence, compact/hybrid/window syntax, agent overlays,
     provider profiles, roles, tool windows, sidebar config.
6. Runtime State And Storage
   - `.ccb` authority files, runtime records, provider-state, shared cache,
     diagnostics, evidence/residue classification.
7. Communication Logic
   - full ask flow, mailbox model, queue/dispatcher, callback, artifact
     transport, watch/pend/trace, reply polling, failure and retry paths.
8. Provider Integration
   - provider registry, launchers, session binding, execution adapters,
     provider-specific reply detection, managed homes.
9. Memory, Rolepacks, Skills, And Commands
   - project memory, provider-native memory, role memory, inherited skills,
     command projection, role store and locks.
10. Diagnostics And Observability
    - doctor, logs, diagnostics bundle, project view, sidebar activity, runtime
      health, generated evidence.
11. Testing And Source Runtime Validation
    - unit test structure, external `ccb_test` validation, source/runtime
      isolation, manual test logs.
12. Architecture Risks And Maintenance
    - Archi hotspots, high-risk components, debt ledgers, refactor guidelines,
      review posture.
13. Appendices
    - command inventory, config grammar reference, storage paths, glossary,
      evidence ledger.

## First Draft Rules

- Prefer direct file references over broad prose.
- Add diagrams only after the corresponding source path has been verified.
- Use generated Archi/Hippo output as a navigational aid, not as standalone
  authority.
- Mark uncertain behavior with an evidence gap instead of smoothing it over.

