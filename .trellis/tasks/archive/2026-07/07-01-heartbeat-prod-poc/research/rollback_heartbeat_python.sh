#!/usr/bin/env bash
# One-command rollback to the Python heartbeat implementation.
set -euo pipefail
SHIM=/root/.local/share/codex-dual/lib/heartbeat/__init__.py
BACKUP=/root/.local/share/codex-dual/lib/heartbeat/__init__.py.python-backup

if [[ ! -f "$BACKUP" ]]; then
    echo "Backup not found: $BACKUP" >&2
    exit 1
fi

cp "$BACKUP" "$SHIM"
echo "Restored Python heartbeat __init__.py"

CCBD_PID=$(pgrep -f 'ccbd/main.py --project /home/agnitum/o13' | head -n 1 || true)
if [[ -n "$CCBD_PID" ]]; then
    echo "Restarting ccbd main (PID $CCBD_PID) to load Python backend..."
    kill "$CCBD_PID"
    sleep 3
    NEW_PID=$(pgrep -f 'ccbd/main.py --project /home/agnitum/o13' | head -n 1 || true)
    echo "New ccbd main PID: ${NEW_PID:-unknown}"
else
    echo "No running ccbd main found"
fi
