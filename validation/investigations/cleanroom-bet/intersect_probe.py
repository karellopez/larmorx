# SPDX-License-Identifier: Apache-2.0
"""Which triangle pairs of a mesh FSL calls self-intersecting do intersect (black-box): every
pair's segment-triangle crossings, for pairs sharing no vertex and for pairs sharing one, on
``bet2 -e``'s mesh file. ``python -P intersect_probe.py <mesh.vtk>``."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from raster_probe import read_vtk


def seg_tri(p, q, t, eps=0.0):
    d = q - p
    e1, e2 = t[1] - t[0], t[2] - t[0]
    h = np.cross(d, e2)
    a = e1 @ h
    if abs(a) < 1e-15:
        return False
    f = 1 / a
    s = p - t[0]
    u = f * (s @ h)
    if u < -eps or u > 1 + eps:
        return False
    qv = np.cross(s, e1)
    v = f * (d @ qv)
    if v < -eps or u + v > 1 + eps:
        return False
    w = f * (e2 @ qv)
    return -eps <= w <= 1 + eps


def main(argv):
    pts, tri = read_vtk(Path(argv[0]))
    T = pts[tri]
    lo, hi = T.min(1), T.max(1)
    found = {"none": 0, "one": 0}
    for i in range(len(tri)):
        ov = np.all((lo[i] <= hi) & (lo <= hi[i]), axis=1)
        for j in np.nonzero(ov)[0]:
            if j <= i:
                continue
            shared = len(set(tri[i]) & set(tri[j]))
            if shared >= 2:
                continue
            a, b = T[i], T[j]
            hit = any(seg_tri(a[k], a[(k + 1) % 3], b) for k in range(3)) or any(
                seg_tri(b[k], b[(k + 1) % 3], a) for k in range(3)
            )
            if hit:
                found["none" if shared == 0 else "one"] += 1
    print("crossing pairs: sharing no vertex", found["none"], "; sharing one vertex", found["one"])


if __name__ == "__main__":
    main(sys.argv[1:])
