# Mailbox Internal Design References

Date: 2026-05-27

Role: Reference index
Status: Reference
Read when: Changing provider activity, message-bureau retry, mailbox summary
authority, or any status path that might mix provider execution facts with
mailbox policy.

## Purpose

This note records the existing internal mailbox/message-bureau design documents
that should be treated as reference material for future provider-activity work.

This is not about the sidebar Comms UI. Comms rows are one rendered workflow
view. The underlying design reference is the mailbox kernel and information
management layer: message, attempt, inbound event, reply, retry, summary, and
diagnostics.

## Existing Reference Documents

- [agent-mailbox-kernel-design.md](../../../../agent-mailbox-kernel-design.md)
  defines the next-generation mailbox kernel blueprint. Key rules: public
  interface is agent-first, inbox consumption is serial per agent, send and
  receive are separate lanes, replies return as inbound events, provider/backend
  reports facts only, and every retry creates a new attempt without overwriting
  old attempt or job history.
- [agent-message-management-roadmap.md](../../../../agent-message-management-roadmap.md)
  defines the information-management layer boundary. Provider/backend owns
  startup, provider-native progress, completion evidence, runtime/session/pane
  health, and native failure facts. The bureau owns queue policy, wait semantics,
  retry/resubmit policy, reply aggregation, lineage/correlation,
  operator-facing state, dead-letter, and recovery workflow.
- [agent-message-timeout-retry-contract.md](../../../../agent-message-timeout-retry-contract.md)
  defines timeout and retry behavior: startup binding failure must terminalize,
  timeout means inspect rather than blind failure, retry preserves context with
  `continue` when prior context exists, and timeout does not auto-retry by
  default.
- [ccbd-p3-p4-mailbox-cli-plan.md](../../../../ccbd-p3-p4-mailbox-cli-plan.md)
  defines mailbox summary/read-model stabilization. Append-only mailbox ledgers
  remain durable evidence, but routine observer reads should use one
  authoritative summary/head model instead of divergent full-history scans.
- [ccbd-ask-submit-fastpath-plan.md](../../../../ccbd-ask-submit-fastpath-plan.md)
  records the shared status lineage chain:
  `submission -> message -> attempt -> job -> reply -> mailbox event`.
- [ccbd-diagnostics-contract.md](../../../../ccbd-diagnostics-contract.md)
  requires `doctor` to surface mailbox summary authority, freshness, head/queue
  facts, and summary-vs-ledger consistency without mutating mailbox artifacts.

## Boundary For Provider Activity

Provider-native activity should feed the provider-fact side of the design, not
replace mailbox policy.

Target interpretation:

```text
provider-native hooks/session/app-server signals
  -> normalized provider activity / health fact
  -> project_view execution status and, later, mailbox liveness input
```

The bureau remains responsible for policy:

```text
message + attempt + job + reply + provider facts
  -> AttemptState / MessageState / retry / dead-letter / recovery
```

Therefore:

- sidebar status may use provider activity as execution-state truth;
- message-bureau retry must still be driven by terminal decisions and policy;
- a sticky provider `failed` status should not by itself mutate mailbox state;
- if provider activity later informs retry/recovery, it should enter through a
  normalized provider-health/liveness fact path, not by making the sidebar or
  Comms view authoritative.
