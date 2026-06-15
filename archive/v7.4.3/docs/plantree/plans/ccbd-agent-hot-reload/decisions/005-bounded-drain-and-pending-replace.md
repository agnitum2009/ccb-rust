# Bounded Drain And Pending Replace

Date: 2026-05-29

## Context

Dynamic unload and replace can wait on busy agents. If an agent stays busy
forever, unbounded `draining` or `pending_replace` state can block future reload
work and retain runtime resources indefinitely.

## Decision

Dynamic unload and replace require explicit bounds before they are exposed:

- drain timeout;
- pending queue length;
- pending age limit;
- duplicate/supersede rules for the same agent slot;
- stable rejection when limits are reached.

Busy replacement is the last dynamic lifecycle class to expose.

## Consequences

The daemon cannot accumulate unbounded pending reload state. A busy agent may
delay its own unload or replacement, but it must not make the whole project
unrecoverable or cause indefinite resource growth.

Related topics:

- [dynamic-unload-and-replace.md](../topics/dynamic-unload-and-replace.md)
- [execution-plan.md](../topics/execution-plan.md)
