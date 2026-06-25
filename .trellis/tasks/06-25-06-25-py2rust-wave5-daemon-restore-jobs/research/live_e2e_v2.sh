#!/usr/bin/env bash
set -euo pipefail

# Live e2e: provider job is running when daemon shuts down; after restart,
# `ccbr trace` still shows the running job, and heartbeat eventually completes it.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(mktemp -d /tmp/ccbr-restore-live-XXXXXX)"
mkdir -p "$PROJECT_DIR/.ccbr"
cat > "$PROJECT_DIR/.ccbr/ccbr.config" <<'EOF'
version = 2
default_agents = ["codex"]

[agents.codex]
provider = "codex"
target = "codex"

[windows]
main = "codex:codex"
EOF

CCBR="/home/agnitum/ccb/rust/target/debug/ccbr"
CCBRD="/home/agnitum/ccb/rust/target/debug/ccbrd"
export CCBR_SOURCE_RUNTIME_OK=1
export CODEX_START_CMD="sh -c 'exec sleep 60'"

log() {
  echo "[$(date -Iseconds)] $*" | tee -a "$SCRIPT_DIR/live_e2e_v2.log"
}

DAEMON_PID=""

start_daemon() {
  log "starting ccbrd in background"
  rm -f "$PROJECT_DIR/.ccbr/ccbrd/ccbrd.sock"
  "$CCBRD" "$PROJECT_DIR" > "$SCRIPT_DIR/daemon_$1.log" 2>&1 &
  DAEMON_PID=$!
  # Wait for socket.
  for i in $(seq 1 30); do
    if [ -S "$PROJECT_DIR/.ccbr/ccbrd/ccbrd.sock" ]; then
      log "ccbrd ready (pid=$DAEMON_PID)"
      return 0
    fi
    sleep 0.2
  done
  log "ERROR: ccbrd socket did not appear"
  return 1
}

stop_daemon() {
  log "stopping ccbrd via RPC"
  run_ccbr shutdown | tee -a "$SCRIPT_DIR/step_shutdown_$1.log"
  # Wait for running-jobs.json to be written (persist happens at shutdown start).
  for i in $(seq 1 30); do
    if [ -f "$PROJECT_DIR/.ccbr/ccbrd/running-jobs.json" ]; then
      log "running-jobs.json persisted"
      break
    fi
    sleep 0.2
  done
  # Wait for process to exit (graceful shutdown may take a while because the
  # pane process is kept alive).
  for i in $(seq 1 60); do
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
      DAEMON_PID=""
      rm -f "$PROJECT_DIR/.ccbr/ccbrd/ccbrd.sock"
      return 0
    fi
    sleep 0.5
  done
  log "WARN: ccbrd did not exit, killing"
  kill -9 "$DAEMON_PID" 2>/dev/null || true
  wait "$DAEMON_PID" 2>/dev/null || true
  DAEMON_PID=""
  rm -f "$PROJECT_DIR/.ccbr/ccbrd/ccbrd.sock"
}

run_ccbr() {
  "$CCBR" --project "$PROJECT_DIR" "$@"
}

get_job_id() {
  run_ccbr trace codex 2>/dev/null | grep -oE 'job_[0-9a-f]+' | head -1
}

write_session_event() {
  local event="$1"
  echo "$event" >> "$PROJECT_DIR/codex-session.jsonl"
}

cleanup() {
  log "cleaning up project $PROJECT_DIR"
  run_ccbr shutdown >/dev/null 2>&1 || true
  "$CCBR" --project "$PROJECT_DIR" stop-all --force >/dev/null 2>&1 || true
  bash /home/agnitum/ccb/scripts/ccbr-test-cleanup.sh >/dev/null 2>&1 || true
  rm -rf "$PROJECT_DIR"
}
trap cleanup EXIT

log "PROJECT_DIR=$PROJECT_DIR"

# 1. Start daemon and agent.
log "step1: start daemon + codex agent"
start_daemon first
run_ccbr start codex | tee -a "$SCRIPT_DIR/step1_start.log"

# 2. Seed auth.json so ask can reach the pane.
mkdir -p "$PROJECT_DIR/.ccbr/runtime/codex/home"
echo '{}' > "$PROJECT_DIR/.ccbr/runtime/codex/home/auth.json"

# 3. Submit ask.
log "step2: ask codex hello"
run_ccbr ask codex hello | tee -a "$SCRIPT_DIR/step2_ask.log"

JOB_ID=$(get_job_id)
log "job_id=$JOB_ID"
if [ -z "$JOB_ID" ]; then
  log "ERROR: no job_id found after ask"
  exit 1
fi

# 4. Pull the exact request anchor from the persisted execution state so the
#    synthetic session events match what the codex adapter expects.
EXEC_STATE="$PROJECT_DIR/.ccbr/ccbrd/executions/${JOB_ID}.json"
for i in $(seq 1 30); do
  if [ -f "$EXEC_STATE" ]; then
    break
  fi
  sleep 0.2
done
if [ ! -f "$EXEC_STATE" ]; then
  log "ERROR: execution state not found at $EXEC_STATE"
  exit 1
fi
ANCHOR=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d.get('submission',{}).get('runtime_state',{}).get('request_anchor',''))" "$EXEC_STATE")
if [ -z "$ANCHOR" ]; then
  log "ERROR: could not extract request_anchor from execution state"
  cat "$EXEC_STATE" | tee -a "$SCRIPT_DIR/exec_state.log"
  exit 1
fi
USER_TEXT="req- ${ANCHOR}"
log "extracted anchor=$ANCHOR"

# 5. Write an incomplete session event so the adapter sees an anchor but no terminal decision.
write_session_event "{\"type\":\"event_msg\",\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"payload\":{\"type\":\"user_message\",\"message\":\"${USER_TEXT}\"}}"

# 5. Poll trace until the job is running.
log "step3: poll until running"
for i in $(seq 1 30); do
  STATUS=$(run_ccbr trace codex 2>/dev/null | grep "$JOB_ID" | awk '{print $3}' | tr -d '[]')
  log "poll $i: status=$STATUS"
  if [ "${STATUS:-}" = "running" ]; then
    break
  fi
  sleep 1
done

if [ "${STATUS:-}" != "running" ]; then
  log "ERROR: job did not reach running before shutdown (status=$STATUS)"
  run_ccbr trace codex | tee -a "$SCRIPT_DIR/step3_trace.log"
  exit 1
fi

log "step3_ok: job is running, status=$STATUS"
run_ccbr trace codex > "$SCRIPT_DIR/step3_trace_running.log"

# 6. Graceful shutdown to persist running jobs.
log "step4: shutdown daemon"
stop_daemon first
sleep 1

# 7. Restart daemon and agent.
log "step5: restart daemon + codex agent"
start_daemon second
run_ccbr start codex | tee -a "$SCRIPT_DIR/step5_restart.log"
sleep 2

# 8. Trace must show the same job still running.
log "step6: trace after restart"
run_ccbr trace codex | tee "$SCRIPT_DIR/step6_trace_after_restart.log"
AFTER_JOB_ID=$(get_job_id)
AFTER_STATUS=$(grep "$JOB_ID" "$SCRIPT_DIR/step6_trace_after_restart.log" | awk '{print $3}' | tr -d '[]' || true)
log "after restart: job_id=$AFTER_JOB_ID status=$AFTER_STATUS"

if [ "$AFTER_JOB_ID" != "$JOB_ID" ]; then
  log "ERROR: job did not survive restart (expected $JOB_ID, got $AFTER_JOB_ID)"
  exit 1
fi
if [ "${AFTER_STATUS:-}" != "running" ]; then
  log "ERROR: job is not running after restart (status=$AFTER_STATUS)"
  exit 1
fi

log "step6_ok: running job survived daemon restart"

# 9. Complete the job by writing a terminal event.
write_session_event "{\"type\":\"event_msg\",\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"payload\":{\"type\":\"task_complete\",\"last_agent_message\":\"done\"}}"

# 10. Poll until terminal.
log "step7: poll until terminal"
for i in $(seq 1 30); do
  FINAL_STATUS=$(run_ccbr trace codex 2>/dev/null | grep "$JOB_ID" | awk '{print $3}' | tr -d '[]' || true)
  log "poll $i: final_status=$FINAL_STATUS"
  case "${FINAL_STATUS:-}" in
    completed|failed|cancelled) break ;;
  esac
  sleep 1
done

run_ccbr trace codex > "$SCRIPT_DIR/step7_trace_final.log"
if [ "${FINAL_STATUS:-}" != "completed" ]; then
  log "ERROR: job did not complete (status=$FINAL_STATUS)"
  exit 1
fi

log "step7_ok: restored job completed without resubmission"
log "LIVE_E2E_PASSED"
