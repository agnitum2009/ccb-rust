"""Worker for heartbeat_prod_poc.py — run in a fresh subprocess."""
from __future__ import annotations

import json
import sys
import time
from dataclasses import asdict


def _rss_kb() -> int | None:
    try:
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except Exception:
        return None
    return None


def make_cases():
    from heartbeat import HeartbeatPolicy, HeartbeatState

    policy = HeartbeatPolicy(
        silence_start_after_s=30.0, repeat_interval_s=60.0, max_notice_count=3
    )
    base_ts = "2026-06-20T05:00:00Z"

    states = [
        None,
        HeartbeatState(
            subject_kind="job_progress",
            subject_id="j1",
            owner="codex",
            last_progress_at=base_ts,
            last_notice_at=None,
            heartbeat_started_at=None,
            notice_count=0,
            updated_at=base_ts,
        ),
        HeartbeatState(
            subject_kind="job_progress",
            subject_id="j1",
            owner="codex",
            last_progress_at=base_ts,
            last_notice_at="2026-06-20T05:00:45Z",
            heartbeat_started_at="2026-06-20T05:00:45Z",
            notice_count=1,
            updated_at="2026-06-20T05:00:45Z",
        ),
        HeartbeatState(
            subject_kind="job_progress",
            subject_id="j1",
            owner="codex",
            last_progress_at=base_ts,
            last_notice_at="2026-06-20T05:00:45Z",
            heartbeat_started_at="2026-06-20T05:00:45Z",
            notice_count=3,
            updated_at="2026-06-20T05:01:50Z",
        ),
    ]

    nows = [
        "2026-06-20T05:00:05Z",  # silence < 30s -> idle
        "2026-06-20T05:00:35Z",  # silence > 30s -> enter
        "2026-06-20T05:01:50Z",  # repeat interval elapsed -> repeat
        "2026-06-20T05:03:00Z",  # max notices reached -> idle
        "2026-06-20T05:00:01Z",  # progress advanced -> reset
    ]
    return policy, states, nows


def run(backend_name: str) -> dict:
    from heartbeat import evaluate_heartbeat

    import_rss = _rss_kb()

    policy, states, nows = make_cases()

    results = []
    for state in states:
        for now in nows:
            ns, dec = evaluate_heartbeat(
                policy=policy,
                subject_kind="job_progress",
                subject_id="j1",
                owner="codex",
                observed_last_progress_at="2026-06-20T05:00:00Z",
                now=now,
                state=state,
            )
            results.append(
                {
                    "state": state.to_record() if state else None,
                    "now": now,
                    "next_state": ns.to_record(),
                    "decision": asdict(dec),
                }
            )

    # Benchmark: run the full matrix many times.
    iterations = 100_000
    start = time.perf_counter_ns()
    for _ in range(iterations):
        for state in states:
            for now in nows:
                evaluate_heartbeat(
                    policy=policy,
                    subject_kind="job_progress",
                    subject_id="j1",
                    owner="codex",
                    observed_last_progress_at="2026-06-20T05:00:00Z",
                    now=now,
                    state=state,
                )
    elapsed = time.perf_counter_ns() - start
    total_calls = iterations * len(states) * len(nows)
    ns_per_tick = elapsed / total_calls

    return {
        "backend": backend_name,
        "import_rss_kb": import_rss,
        "ns_per_tick": ns_per_tick,
        "results": results,
    }


if __name__ == "__main__":
    backend = sys.argv[1]
    print(json.dumps(run(backend), indent=2))
