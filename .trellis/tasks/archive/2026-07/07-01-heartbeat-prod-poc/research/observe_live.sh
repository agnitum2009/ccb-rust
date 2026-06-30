#!/usr/bin/env bash
# Live observation sampler for the heartbeat Rust PoC.
set -euo pipefail

OUT="/home/agnitum/ccb/.trellis/tasks/07-01-heartbeat-prod-poc/research/live_observation.log"
STDERR_LOG="/home/agnitum/o13/.ccb/ccbd/ccbd.stderr.log"
LIFECYCLE_LOG="/home/agnitum/o13/.ccb/ccbd/lifecycle.jsonl"
SUPERVISION_LOG="/home/agnitum/o13/.ccb/ccbd/supervision.jsonl"
ITERATIONS="${1:-30}"
INTERVAL_SEC="${2:-60}"

echo "=== Heartbeat Rust PoC live observation ===" > "$OUT"
echo "start: $(date -Iseconds)" >> "$OUT"
echo "iterations: $ITERATIONS, interval: ${INTERVAL_SEC}s" >> "$OUT"
echo "" >> "$OUT"

# Record baseline log positions.
STDERR_POS=$(wc -c < "$STDERR_LOG" 2>/dev/null || echo 0)
LIFECYCLE_POS=$(wc -c < "$LIFECYCLE_LOG" 2>/dev/null || echo 0)
SUPERVISION_POS=$(wc -c < "$SUPERVISION_LOG" 2>/dev/null || echo 0)

printf "%-24s %10s %10s %10s\n" "timestamp" "ccbd_pid" "ccbd_rss" "keeper_rss" >> "$OUT"

for ((i=1; i<=ITERATIONS; i++)); do
    CCBD_PID=$(pgrep -f 'ccbd/main.py --project /home/agnitum/o13' | head -n 1 || true)
    KEEPER_PID=$(pgrep -f 'ccbd/keeper_main.py --project /home/agnitum/o13' | head -n 1 || true)
    if [[ -n "$CCBD_PID" ]]; then
        CCBD_RSS=$(ps -o rss= -p "$CCBD_PID" 2>/dev/null || echo NA)
    else
        CCBD_PID=NA
        CCBD_RSS=NA
    fi
    if [[ -n "$KEEPER_PID" ]]; then
        KEEPER_RSS=$(ps -o rss= -p "$KEEPER_PID" 2>/dev/null || echo NA)
    else
        KEEPER_PID=NA
        KEEPER_RSS=NA
    fi
    printf "%-24s %10s %10s %10s\n" "$(date -Iseconds)" "$CCBD_PID" "$CCBD_RSS" "$KEEPER_RSS" >> "$OUT"

    # Check for new heartbeat-related errors in the relevant logs.
    for pair in "$STDERR_LOG:$STDERR_POS" "$LIFECYCLE_LOG:$LIFECYCLE_POS" "$SUPERVISION_LOG:$SUPERVISION_POS"; do
        log=${pair%%:*}
        pos=${pair##*:}
        if [[ -f "$log" ]]; then
            new_errors=$(tail -c +$((pos + 1)) "$log" 2>/dev/null | grep -iE 'heartbeat|ccb_py_heartbeat' || true)
            if [[ -n "$new_errors" ]]; then
                echo "--- new errors in $log at $(date -Iseconds) ---" >> "$OUT"
                echo "$new_errors" >> "$OUT"
            fi
        fi
    done

    # Update positions.
    STDERR_POS=$(wc -c < "$STDERR_LOG" 2>/dev/null || echo 0)
    LIFECYCLE_POS=$(wc -c < "$LIFECYCLE_LOG" 2>/dev/null || echo 0)
    SUPERVISION_POS=$(wc -c < "$SUPERVISION_LOG" 2>/dev/null || echo 0)

    if (( i < ITERATIONS )); then
        sleep "$INTERVAL_SEC"
    fi
done

echo "" >> "$OUT"
echo "end: $(date -Iseconds)" >> "$OUT"
echo "=== observation complete ===" >> "$OUT"
