# Managed Provider Completion Reliability Open Questions

Date: 2026-06-12

1. Should `completion_timeout` with a non-empty provider reply stay
   `incomplete` with stronger diagnostics, or can any provider-specific case be
   safely reclassified as `completed`?
2. Should Claude `stop_sequence` ever be treated as a completed turn, and what
   additional evidence would be required?
3. Should Claude `max_tokens` terminal evidence become
   `incomplete/max_tokens` with partial reply, instead of waiting for timeout?
4. Where should maintenance heartbeat read provider-finished-but-not-terminal
   evidence without becoming a completion authority itself?
5. When Codex binding evidence is stale before prompt delivery, should CCB
   return a retryable runtime error only, or may it automatically restart the
   affected worker when the queue is idle?
6. If `codex.pid` is stale but the current pane process, session log, and
   activity evidence are coherent, is safe rebind allowed, or should the first
   repair always require `ccb restart <agent>`?
7. After `delivery_anchor_missing` with no anchor and no reply evidence, should
   retry stay explicitly user/operator initiated to avoid duplicate side
   effects?
8. What exact `ccb ps` / doctor wording should distinguish "mailbox consumed"
   from "provider accepted the prompt" without implying message loss?
