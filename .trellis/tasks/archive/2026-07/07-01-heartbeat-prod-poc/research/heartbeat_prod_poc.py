"""Heartbeat PyO3 production PoC harness.

Runs the same heartbeat state transitions through both the Python and Rust
backends and compares outputs, import memory, and per-tick CPU cost.

Usage:
    PYTHONPATH=/root/.local/share/codex-dual/lib python3 heartbeat_prod_poc.py
"""
from __future__ import annotations

import os
import sys
import time
from dataclasses import asdict
from pathlib import Path

LIB = Path("/root/.local/share/codex-dual/lib")


def _rss_kb() -> int | None:
    try:
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except Exception:
        return None
    return None


def run_backend(name: str) -> dict:
    # Force a fresh interpreter subprocess for each backend so import memory is
    # isolated and Python import caches do not leak across backends.
    env = os.environ.copy()
    if name == "rust":
        env["CCB_HEARTBEAT_RUST"] = "1"
    else:
        env.pop("CCB_HEARTBEAT_RUST", None)
    env["PYTHONPATH"] = str(LIB)

    script = Path(__file__).with_suffix(".worker.py")
    import subprocess

    proc = subprocess.run(
        [sys.executable, str(script), name],
        env=env,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"{name} worker failed:\n{proc.stderr}")
    import json

    return json.loads(proc.stdout)


def main() -> int:
    print("=== Heartbeat PyO3 Production PoC ===")
    print(f"lib path: {LIB}")
    print(f"python:   {sys.executable}")
    print()

    py = run_backend("python")
    rs = run_backend("rust")

    print(f"Python import RSS: {py['import_rss_kb']} kB")
    print(f"Rust   import RSS: {rs['import_rss_kb']} kB")
    print(f"Import delta:      {rs['import_rss_kb'] - py['import_rss_kb']:+,d} kB")
    print()

    print(f"Python per-tick:   {py['ns_per_tick']:.0f} ns")
    print(f"Rust   per-tick:   {rs['ns_per_tick']:.0f} ns")
    if py["ns_per_tick"] > 0:
        delta = (rs["ns_per_tick"] - py["ns_per_tick"]) / py["ns_per_tick"] * 100
        print(f"CPU delta:         {delta:+.1f}%")
    print()

    if py["results"] != rs["results"]:
        print("RESULT MISMATCH")
        for i, (p, r) in enumerate(zip(py["results"], rs["results"])):
            if p != r:
                print(f"  case {i}: python={p} rust={r}")
        return 1

    print(f"All {len(py['results'])} cases matched between Python and Rust.")
    print()
    print("PoC result: PASS (no behavioral drift)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
