# SPDX-License-Identifier: Apache-2.0
"""Which of larmorx's programs run parallel work outside their own pool of the call's thread
count: every registered program over its code paths (``larmorx_validation.parity.thread_pools``)
at one thread, as a table. The permanent check of the same cases is
``tests/parity/test_thread_pools.py``.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P global_pool_probe.py <dir> [--gpl <larmorx-gpl>] [--nc <larmorx-nc>]

Exits 1 when a program starts a thread outside its pool.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def main() -> int:
    from larmorx_validation.parity import thread_pools

    args = sys.argv[1:]
    d = Path(args.pop(0)).resolve()
    gpl = args[args.index("--gpl") + 1] if "--gpl" in args else None
    nc = args[args.index("--nc") + 1] if "--nc" in args else None
    d.mkdir(parents=True, exist_ok=True)
    print(f"CPUs this process may use: {len(os.sched_getaffinity(0))}")
    print("| Program and path | exit | `larmorx-<n>` pool threads | other threads (by name) |")
    print("|---|---|---|---|")
    bad = total = 0
    for p in thread_pools.programs(d, gpl, nc):
        code, pool, other, err = thread_pools.probe(p, d)
        names = ", ".join(f"{k} x{v}" for k, v in sorted(other.items())) or "none"
        total += 1
        bad += bool(other)
        print(f"| {p.name} | {code} | {pool} | {names} |", flush=True)
        if code != 0:
            print(f"    ({err.strip().splitlines()[-1] if err.strip() else ''})", flush=True)
    print(f"\n{bad} of {total} start threads outside their pool")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
