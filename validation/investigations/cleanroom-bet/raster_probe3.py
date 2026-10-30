# SPDX-License-Identifier: Apache-2.0
"""More candidate rasterisations against FSL's outline (black-box; see raster_probe.py): every
choice of the first edge and its direction, accumulated or computed parameters, and the
inner line's length. ``python -P raster_probe3.py <fsl run dir> <input image>``."""

from __future__ import annotations

import itertools
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from raster_probe import read_vtk  # noqa: E402


def draw(tri_pts, order, step, mode):
    a, b, c = (tri_pts[i] for i in order)
    out = []
    L = np.linalg.norm(b - a)
    if mode == "bary":
        L2 = np.linalg.norm(c - a)
        j = 0.0
        while j <= 1:
            k = 0.0
            while j + k <= 1:
                out.append(a + j * (b - a) + k * (c - a))
                k += step / L2
            j += step / L
        return out
    if mode in ("mm", "mm-rev"):
        # along unit directions in mm: i = 0, step, ... <= length
        n = (b - a) / L
        j = 0.0
        while j <= L:
            e = a + j * n
            if mode == "mm":
                s0, s1 = e, c
            else:
                s0, s1 = c, e
            L2 = np.linalg.norm(s1 - s0)
            n2 = (s1 - s0) / L2 if L2 > 0 else s1 - s0
            k = 0.0
            while k <= L2:
                out.append(s0 + k * n2)
                k += step
            j += step
        return out
    inc = step / L
    j = 0.0
    while j <= 1:
        e = a + j * (b - a)
        L2 = np.linalg.norm(c - e) if mode != "edgelen" else np.linalg.norm(c - a)
        inc2 = step / L2 if L2 > 0 else 2
        k = 0.0
        while k <= 1:
            out.append(e + k * (c - e) if mode != "acc-rev" else c + k * (e - c))
            k += inc2
        j += inc
    return out


def main(argv):
    d, inp = Path(argv[0]), argv[1]
    img = nib.load(inp)
    shape = img.shape[:3]
    vox = np.array([abs(float(np.float32(z))) for z in img.header.get_zooms()[:3]])
    pts, tri = read_vtk(d / "out_mesh.vtk")
    over = np.asanyarray(nib.load(d / "out_overlay.nii.gz").dataobj)
    data = np.asanyarray(img.dataobj)
    fline = (over == over.max()) & (data != over.max())
    step = vox.min() / 2
    sub = tri[:: max(1, len(tri) // 600)]  # a sample of the triangles first
    for order in itertools.permutations(range(3)):
        for mode in ("acc-rev", "mm", "mm-rev"):
            allp = np.array([p for t in sub for p in draw(pts[t], order, step, mode)])
            idx = np.trunc(allp / vox + 0.5).astype(int)
            hit = fline[idx[:, 0], idx[:, 1], idx[:, 2]]
            print(order, mode, "points off FSL's outline:", int((~hit).sum()), "of", len(hit))


if __name__ == "__main__":
    main(sys.argv[1:])
