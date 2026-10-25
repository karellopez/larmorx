# SPDX-License-Identifier: Apache-2.0
"""The resident memory of one command over its run: ``/proc/<pid>/status`` (``VmRSS``) read
every half millisecond while it runs, printed where it changes by more than a step, so the
phases of a program's memory (reading, working images, writing) can be told apart and set
against its code (random testing, 2026-10-25: larmorx's N4BiasFieldCorrection and DenoiseImage
against ANTs' peak).

    python -P validation/investigations/difftest/rss_timeline.py [--step-mib 1] -- <command...>

Prints ``seconds  MiB`` lines and the peak (``VmHWM``). Linux only.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import time


def status(pid: int) -> dict[str, int]:
    out = {}
    try:
        with open(f"/proc/{pid}/status", encoding="ascii") as f:
            for line in f:
                key, _, rest = line.partition(":")
                if key in ("VmRSS", "VmHWM"):
                    out[key] = int(rest.split()[0])
    except (OSError, ValueError):
        pass
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--step-mib", type=float, default=1.0)
    ap.add_argument("command", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    cmd = args.command[1:] if args.command[:1] == ["--"] else args.command
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    t0 = time.monotonic()
    last = -1e9
    hwm = 0
    rows = []
    while p.poll() is None:
        s = status(p.pid)
        if "VmRSS" in s:
            mib = s["VmRSS"] / 1024
            hwm = max(hwm, s.get("VmHWM", 0))
            if abs(mib - last) >= args.step_mib:
                rows.append((time.monotonic() - t0, mib))
                last = mib
        time.sleep(0.0005)
    err = p.stderr.read().decode("utf-8", "replace") if p.stderr else ""
    for t, mib in rows:
        print(f"{t:8.3f} s  {mib:8.1f} MiB")
    print(f"exit {p.returncode}; peak seen {hwm / 1024:.1f} MiB; {err.strip()[:300]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
