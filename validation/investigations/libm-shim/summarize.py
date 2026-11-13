# SPDX-License-Identifier: Apache-2.0
"""Sums a libm shim log (one line per process and function: program pid mode function calls
differing) by program and function.

    python -P summarize.py <shim log> [--differing]

One line per (program, mode, function): processes, calls, differing; --differing keeps only the
functions with differing calls (atan2's table, 207 differing doubles per process, is shown apart).
"""

from __future__ import annotations

import collections
import sys


def main() -> int:
    path = sys.argv[1]
    only = "--differing" in sys.argv
    acc: dict[tuple[str, str, str], list[int]] = collections.defaultdict(lambda: [0, 0, 0])
    table = collections.Counter()
    with open(path, encoding="utf-8") as f:
        for line in f:
            parts = line.split()
            if len(parts) != 6:
                continue
            prog, _pid, mode, fn, calls, diff = parts
            if fn == "atan2" and calls == "100001" and diff == "207":
                table[prog] += 1
                continue
            a = acc[(prog, mode, fn)]
            a[0] += 1
            a[1] += int(calls)
            a[2] += int(diff)
    for (prog, mode, fn), (procs, calls, diff) in sorted(acc.items()):
        if only and not diff:
            continue
        print(
            f"{prog:32s} {mode:5s} {fn:8s} {procs:4d} processes {calls:14,d} calls {diff:12,d} differ"
        )
    if table:
        progs = ", ".join(f"{p} {n}" for p, n in sorted(table.items()))
        print(f"atan2's table only (100,001 calls, 207 differing doubles, per process): {progs}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
