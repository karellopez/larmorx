# SPDX-License-Identifier: Apache-2.0
"""cmpmesh.py <ref.vtk> <other.vtk>...: per-vertex distances (p95, max) and identical lines."""

import sys
from pathlib import Path

import numpy as np


def read(p):
    lines = Path(p).read_text().splitlines()
    n = int(lines[4].split()[1])
    pts = np.array([[float(x) for x in lines[5 + i].split()] for i in range(n)])
    return lines, pts


ref_lines, ref = read(sys.argv[1])
for p in sys.argv[2:]:
    lines, pts = read(p)
    d = np.linalg.norm(pts - ref, axis=1)
    same = sum(a == b for a, b in zip(lines, ref_lines, strict=True))
    print(
        f"{p}: p95 {np.percentile(d, 95):.4f} max {d.max():.4f} mm; {same} of {len(ref_lines)} lines identical"
    )
