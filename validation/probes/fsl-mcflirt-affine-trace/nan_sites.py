# SPDX-License-Identifier: Apache-2.0
"""The NaN-generation sites ``libnan_trap.so`` recorded (``<dir>/**/trap/*.txt``), symbolized
with their inlined frames against a build with line tables (``CARGO_PROFILE_RELEASE_DEBUG=
line-tables-only``), with counts. Only the arithmetic sites (add, subtract, multiply, divide,
square root) are listed unless ``--all`` is given (conversions and comparisons also raise the
invalid exception).

    python -P nan_sites.py <binary with line tables> <dir> [<dir> ...] [--all]
"""

from __future__ import annotations

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path


def main() -> int:
    binary = sys.argv[1]
    dirs = [a for a in sys.argv[2:] if not a.startswith("--")]
    counts: dict[str, int] = defaultdict(int)
    for d in dirs:
        for f in Path(d).rglob("trap/*.txt"):
            for line in f.read_text().splitlines()[1:]:
                m = re.match(r"(0x[0-9a-f]+) (\d+)( arith)?$", line)
                if m and (m.group(3) or "--all" in sys.argv):
                    counts[m.group(1)] += int(m.group(2))
    if not counts:
        print("no sites")
        return 0
    offs = sorted(counts, key=lambda o: int(o, 16))
    out = subprocess.run(
        ["addr2line", "-e", binary, "-f", "-C", "-i", "-a", *offs],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()
    frames: dict[int, list[str]] = defaultdict(list)
    cur, i = 0, 0
    while i < len(out):
        if out[i].startswith("0x"):
            cur = int(out[i], 16)
            i += 1
            continue
        loc = out[i + 1] if i + 1 < len(out) else ""
        frames[cur].append(f"{loc.split(' (discriminator')[0]}  {out[i][:90]}")
        i += 2
    for o in offs:
        print(f"{o} x{counts[o]}")
        for fr in frames.get(int(o, 16), ["?"])[:4]:
            print("    ", fr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
