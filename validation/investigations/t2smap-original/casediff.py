# SPDX-License-Identifier: Apache-2.0
"""Per case, the lines of report B that differ from report A (``compare.py``'s reports, check
sections left out): ``python -I casediff.py <a.txt> <b.txt> [case,...]``."""

import difflib
import sys


def cases(path):
    out, name = {}, None
    from pathlib import Path

    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if line.startswith("== check "):
            name = None
            continue
        if line.startswith("== "):
            name = line[3:].split(":")[0]
            out[name] = []
        elif name:
            out[name].append(line)
    return out


a, b = cases(sys.argv[1]), cases(sys.argv[2])
only = sys.argv[3].split(",") if len(sys.argv) > 3 else None
for name in a:
    if only and not any(name.startswith(o) or o in name for o in only):
        continue
    d = [
        x
        for x in difflib.unified_diff(a[name], b.get(name, []), lineterm="", n=0)
        if not x.startswith(("---", "+++", "@@"))
    ]
    if d:
        print(f"== {name}")
        for x in d[:40]:
            print("  " + x[:260])
