# SPDX-License-Identifier: Apache-2.0
"""Which rasterisation of the surface reproduces FSL's mask and outline (black-box).

``python -P raster_probe.py <fsl run dir> <input image>``: the run directory holds FSL's
``out_mesh.vtk``, ``out_mask.nii.gz`` and ``out_overlay.nii.gz`` of ``bet2 <input> out -m -o -e``.
Each candidate rasterisation (which edge, how the points along it are spaced, how a point
picks its voxel) is drawn from FSL's own mesh and filled from the centroid; the voxels where
its mask and outline differ from FSL's are counted. Only FSL's outputs are read.
"""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from scipy import ndimage


def read_vtk(p: Path):
    w = p.read_text().split()
    k = w.index("POINTS")
    n = int(w[k + 1])
    pts = np.array(w[k + 3 : k + 3 + 3 * n], float).reshape(n, 3)
    k = w.index("POLYGONS")
    m = int(w[k + 1])
    tri = np.array(w[k + 3 : k + 3 + 4 * m], int).reshape(m, 4)[:, 1:]
    return pts, tri


def points(tri_pts, step, edge, spacing):
    a, b, c = tri_pts
    if edge == 1:
        a, b, c = b, c, a
    elif edge == 2:
        a, b, c = c, a, b
    out = []
    L = np.linalg.norm(b - a)
    if spacing == "ceil":
        n1 = int(np.ceil(L / step))
        fr1 = [s / n1 for s in range(n1 + 1)] if n1 else [0.0]
        es = [a + (b - a) * f for f in fr1]
    else:  # "step": k*step while <= L, then the end
        es = []
        s = 0.0
        while s <= L:
            es.append(a + (b - a) * (s / L if L else 0))
            s += step
        if spacing == "step+end":
            es.append(b)
    for e in es:
        L2 = np.linalg.norm(c - e)
        if spacing == "ceil":
            n2 = int(np.ceil(L2 / step))
            fr2 = [u / n2 for u in range(n2 + 1)] if n2 else [0.0]
            out.extend(e + (c - e) * f for f in fr2)
        else:
            s = 0.0
            while s <= L2:
                out.append(e + (c - e) * (s / L2 if L2 else 0))
                s += step
            if spacing == "step+end":
                out.append(c)
    return np.array(out)


def main(argv):
    d, inp = Path(argv[0]), argv[1]
    img = nib.load(inp)
    shape = img.shape[:3]
    vox = np.array([abs(float(np.float32(z))) for z in img.header.get_zooms()[:3]])
    pts, tri = read_vtk(d / "out_mesh.vtk")
    fmask = np.asanyarray(nib.load(d / "out_mask.nii.gz").dataobj) != 0
    over = np.asanyarray(nib.load(d / "out_overlay.nii.gz").dataobj)
    fline = over == over.max()
    data = np.asanyarray(img.dataobj)
    fline &= data != over.max()
    step = vox.min() / 2
    for edge in (0, 1, 2):
        for spacing in ("ceil", "step", "step+end"):
            allp = np.concatenate([points(pts[t], step, edge, spacing) for t in tri])
            for rnd in ("floor", "trunc"):
                x = allp / vox + 0.5
                idx = np.floor(x) if rnd == "floor" else np.trunc(x)
                ok = np.all((idx >= 0) & (idx < shape), axis=1)
                idx = idx[ok].astype(int)
                surf = np.zeros(shape, bool)
                surf[idx[:, 0], idx[:, 1], idx[:, 2]] = True
                c = np.trunc(pts.mean(0) / vox).astype(int)
                lab, _ = ndimage.label(~surf, structure=ndimage.generate_binary_structure(3, 1))
                inside = lab == lab[tuple(c)]
                mask = surf | inside
                print(
                    f"edge {edge} {spacing:9s} {rnd}: mask differs in {int((mask != fmask).sum())} "
                    f"(+{int((mask & ~fmask).sum())} -{int((~mask & fmask).sum())}), "
                    f"outline in {int((surf != fline).sum())}"
                )


if __name__ == "__main__":
    main(sys.argv[1:])
