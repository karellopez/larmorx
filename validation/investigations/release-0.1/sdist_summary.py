# SPDX-License-Identifier: Apache-2.0
"""List an sdist's top-level layout (directory counts), its size, and whether forbidden trees are in it."""

import os
import sys
import tarfile
from collections import Counter

for path in sys.argv[1:]:
    with tarfile.open(path) as t:
        names = t.getnames()
        members = t.getmembers()
    top = Counter("/".join(n.split("/")[1:3]) for n in names if n.count("/") >= 2)
    size = sum(m.size for m in members)
    print(
        f"== {os.path.basename(path)}: {os.path.getsize(path) / 1e6:.2f} MB packed, {size / 1e6:.1f} MB unpacked, {len(names)} entries"
    )
    print("   ", ", ".join(f"{k}({v})" for k, v in sorted(top.items())))
    for bad in ("crates-gpl/", "crates-nc/", "fsl-source", "tests/golden", "validation/"):
        hits = [n for n in names if bad in n]
        print(f"    {bad}: {len(hits)}")
    root = sorted({n.split("/")[1] for n in names if n.count("/") >= 1})
    print("    root:", root)
