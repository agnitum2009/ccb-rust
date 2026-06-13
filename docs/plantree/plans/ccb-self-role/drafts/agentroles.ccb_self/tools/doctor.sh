#!/usr/bin/env bash
# Read-only draft doctor helper for agentroles.ccb_self.
# Emits JSON only. Shells out to stable CCB control-plane diagnostics.

set -euo pipefail

CCB_BIN="${CCB_BIN:-$(command -v ccb || true)}"
PROJECT_ARGS=()
if [[ -n "${CCB_ROLE_TOOL_PROJECT_ROOT:-}" ]]; then
    PROJECT_ARGS=("--project" "$CCB_ROLE_TOOL_PROJECT_ROOT")
elif [[ -n "${CCB_PROJECT_ROOT:-}" ]]; then
    PROJECT_ARGS=("--project" "$CCB_PROJECT_ROOT")
fi

GENERATED_AT="$(date -u +%Y-%m-%dT%H:%M:%S%:z)"

tail_output() {
    local text="$1"
    # Last 12000 bytes
    printf '%s' "$text" | tail -c 12000
}

run_cmd() {
    local cmd=("$CCB_BIN" "${PROJECT_ARGS[@]}" "$@")
    local joined
    joined=$(printf ',"%s"' "${cmd[@]}")
    joined="[${joined#,}]"

    local stdout stderr rc status
    stdout=""
    stderr=""
    rc=0
    if ! stdout="$("${cmd[@]}" 2>/tmp/ccb_doctor_stderr.$$)"; then
        rc=$?
    fi
    if [[ -f /tmp/ccb_doctor_stderr.$$ ]]; then
        stderr="$(cat /tmp/ccb_doctor_stderr.$$)"
        rm -f /tmp/ccb_doctor_stderr.$$
    fi

    if [[ $rc -eq 0 ]]; then
        status="ok"
    else
        status="failed"
    fi

    printf '{"command":%s,"status":"%s","returncode":%s,"stdout":"%s","stderr":"%s"}' \
        "$joined" "$status" "$rc" \
        "$(tail_output "$stdout" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\n/\\n/g; s/\t/\\t/g')" \
        "$(tail_output "$stderr" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\n/\\n/g; s/\t/\\t/g')"
}

if [[ -z "$CCB_BIN" ]]; then
    cat <<EOF
{
  "status": "error",
  "summary": "ccb executable not found",
  "generated_at": "$GENERATED_AT",
  "findings": [
    {
      "severity": "error",
      "domain": "tooling",
      "message": "Set CCB_BIN or ensure ccb is on PATH."
    }
  ],
  "evidence": [],
  "recommended_actions": ["Install or expose the CCB CLI, then rerun doctor.sh."]
}
EOF
    exit 2
fi

EVIDENCE=()
for args in "ping ccbd" "doctor" "ps" "queue --detail all" "fault list" "config validate" "reload --dry-run"; do
    # shellcheck disable=SC2086
    EVIDENCE+=("$(run_cmd $args)")
done

EVIDENCE_JSON=$(printf '%s,' "${EVIDENCE[@]}" | sed 's/,$//')

FAILED_COUNT=0
for item in "${EVIDENCE[@]}"; do
    if [[ "$item" != *'"status":"ok"'* ]]; then
        FAILED_COUNT=$((FAILED_COUNT + 1))
    fi
done

if [[ $FAILED_COUNT -eq 0 ]]; then
    cat <<EOF
{
  "status": "ok",
  "summary": "read-only CCB self diagnostics completed",
  "generated_at": "$GENERATED_AT",
  "findings": [],
  "evidence": [$EVIDENCE_JSON],
  "recommended_actions": []
}
EOF
    exit 0
else
    cat <<EOF
{
  "status": "warn",
  "summary": "read-only CCB self diagnostics completed",
  "generated_at": "$GENERATED_AT",
  "findings": [
    {
      "severity": "warn",
      "domain": "ccb-control-plane",
      "message": "One or more read-only diagnostics failed."
    }
  ],
  "evidence": [$EVIDENCE_JSON],
  "recommended_actions": [
    "Inspect failed command output before choosing a repair.",
    "Use CCB control-plane repair commands only after maintenance gates pass."
  ]
}
EOF
    exit 1
fi
