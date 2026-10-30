# SPDX-License-Identifier: Apache-2.0
"""Which reading of the spec's skull search (specs/bet.md §4.7) reproduces FSL's ``_skull``
image (black-box): from FSL's own mesh of ``bet2 <input> out -s -e -m -o`` (its ``out_mesh.vtk``
and ``out_skull.nii.gz``), each variant's skull voxels against FSL's.
``python -P skull_probe.py <fsl run dir> <input image> [t t2 t98]``."""

from __future__ import annotations

import itertools
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from scipy import ndimage

sys.path.insert(0, str(Path(__file__).parent))
from raster_probe import read_vtk  # noqa: E402


def normals(pts, tri):
    # file order (c, a, b) of the internal (a, b, c); outward (c - a) x (b - a)
    a, b, c = pts[tri[:, 1]], pts[tri[:, 2]], pts[tri[:, 0]]
    tn = np.cross(c - a, b - a)
    vn = np.zeros_like(pts)
    for k in range(3):
        np.add.at(vn, tri[:, k], tn)
    return vn / np.linalg.norm(vn, axis=1)[:, None]


def neighbours(n, tri):
    nb = [set() for _ in range(n)]
    for t in tri:
        for i in range(3):
            nb[t[i]].update((t[(i + 1) % 3], t[(i + 2) % 3]))
    return [sorted(s) for s in nb]


def surface(pts, tri, vox, shape):
    from raster_probe import points  # noqa: F401

    step = vox.min() / 2
    out = np.zeros(shape, bool)
    for t in tri:
        a, b, c = pts[t[1]], pts[t[2]], pts[t[0]]

        def seg(p, q):
            d = q - p
            L = np.linalg.norm(d)
            u = d / L if L > 0 else d
            k = 0.0
            res = []
            while k <= L:
                res.append(p + k * u)
                k += step
            return res

        for e in seg(a, c):
            for q in seg(b, e):
                i = np.floor(q / vox + 0.5).astype(int)
                if np.all((i >= 0) & (i < shape)):
                    out[tuple(i)] = True
    return out


MODE = 'voxel'


def main(argv):
    global MODE
    MODE = argv[5] if len(argv) > 5 else 'voxel'
    d, inp = Path(argv[0]), argv[1]
    img = nib.load(inp)
    data = np.asanyarray(img.dataobj, dtype=np.float32)
    shape = data.shape[:3]
    vox = np.array([abs(float(np.float32(z))) for z in img.header.get_zooms()[:3]])
    t, t2, t98 = (float(x) for x in argv[2:5])
    pts, tri = read_vtk(d / "out_mesh.vtk")
    fsk = np.asanyarray(nib.load(d / "out_skull.nii.gz").dataobj) != 0
    print("FSL skull voxels", int(fsk.sum()))
    nrm = normals(pts, tri)
    nb = neighbours(len(pts), tri)
    surf = surface(pts, tri, vox, shape)
    step = vox.min() / 2
    rng = t98 - t2
    # profiles once per (voxel, vertex)
    cast = []
    for v in range(len(pts)):
        p = pts[v]
        h = np.ceil(max(np.linalg.norm(pts[j] - p) for j in nb[v]) / 2)
        lo = np.floor((p - h) / vox).astype(int)
        hi = np.floor((p + h) / vox).astype(int)
        lo, hi = np.maximum(lo, 0), np.minimum(hi, np.array(shape) - 1)
        for i in range(lo[0], hi[0] + 1):
            for j in range(lo[1], hi[1] + 1):
                for k in range(lo[2], hi[2] + 1):
                    if not surf[i, j, k]:
                        continue
                    c = np.array([i, j, k]) * vox
                    dp = np.linalg.norm(c - p)
                    if any(np.linalg.norm(c - pts[q]) < dp for q in nb[v]):
                        continue
                    cast.append((c if MODE == 'voxel' else p, nrm[v]))
    print("profiles", len(cast))
    sgrid = np.arange(-3.0, 30.0, step)
    profs = []
    for c, n in cast:
        q = c[None, :] + sgrid[:, None] * n[None, :]
        profs.append((c, n, ndimage.map_coordinates(data, (q / vox).T, order=1, mode="nearest")))

    def first_max(x):
        return int(np.argmax(x))

    def last_max(x):
        return len(x) - 1 - int(np.argmax(x[::-1]))

    for crit0, ties, dmax_all, rise in itertools.product(
        (False,), ("first",), (False,), ("fwd+1",)
    ):
        pick = first_max if ties == "first" else last_max
        out = np.zeros(shape, bool)
        for c, n, vals in profs:
            s = sgrid
            sel = np.ones_like(s, bool) if dmax_all else s >= 0
            km = int(np.argmax(np.where(sel, vals, -np.inf)))
            if not vals[km] > t:
                continue
            lo = int(np.searchsorted(s, 0.0)) if crit0 else 0
            if km <= lo:
                continue
            crit = s[lo:km] / 30 - vals[lo:km] / rng
            k0 = lo + pick(crit)
            if rise == "fwd+1":
                r = np.diff(vals[k0 : km + 1])
                if len(r) == 0:
                    continue
                kk = k0 + pick(r) + 1
            else:
                if k0 == 0:
                    r = np.diff(vals[k0 : km + 1])
                    kk = k0 + pick(r) + 1
                else:
                    r = vals[k0 : km + 1] - vals[k0 - 1 : km]
                    kk = k0 + pick(r)
            pt = c + s[kk] * n
            x = np.trunc(pt / vox).astype(int)
            if np.all((x >= 0) & (x < shape)):
                out[tuple(x)] = True
        print(f"crit from 0 {crit0}, ties {ties}, d_max over all {dmax_all}, rise {rise}: ours {int(out.sum())} FSL-only {int((fsk & ~out).sum())} ours-only {int((out & ~fsk).sum())}")


if __name__ == "__main__":
    main(sys.argv[1:])
