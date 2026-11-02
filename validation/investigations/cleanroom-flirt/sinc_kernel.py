# SPDX-License-Identifier: Apache-2.0
"""Which windowed-sinc construction gives FSL's `flirt -applyxfm -interp sinc` values bit for bit:
evaluates variants of the kernel (its table, its lookup, the order of the products and sums) at
output voxels of a probe's run and counts the voxels each variant reproduces. A development tool of
the clean-room original (part 1, 2026-11-02); the probe directory comes from probe.py.

    python -P sinc_kernel.py <probe-dir> [voxels]
"""

from __future__ import annotations

import itertools
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

f32 = np.float32


def radiological(img):
    d = np.asanyarray(img.dataobj).astype(np.float32)
    h = img.header
    aff = img.get_sform() if int(h["sform_code"]) > 0 else img.get_qform()
    if (int(h["sform_code"]) > 0 or int(h["qform_code"]) > 0) and np.linalg.det(aff[:3, :3]) > 0:
        d = d[::-1]
    return d


def main() -> None:
    probe = Path(sys.argv[1])
    nvox = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    fd = probe / "fsl"
    src = nib.load(str(fd / "epi.nii"))
    ref = nib.load(str(fd / "t1.nii"))
    out = nib.load(str(fd / "out.nii.gz"))
    v = radiological(src)
    want = radiological(out)
    m = np.loadtxt(fd / "init.mat")
    dv = np.abs(np.array(src.header.get_zooms()[:3], dtype=np.float64))
    dr = np.abs(np.array(ref.header.get_zooms()[:3], dtype=np.float64))
    a = np.diag([*(1 / dv), 1]) @ np.linalg.inv(m) @ np.diag([*dr, 1])
    a = a[:3].astype(np.float32)
    nx, ny, nz = want.shape
    rng = np.random.default_rng(0)
    vox = []
    while len(vox) < nvox:
        x, y, z = rng.integers(0, nx), rng.integers(0, ny), rng.integers(0, nz)
        p = [f32(x) * a[k][0] + f32(z) * a[k][2] + a[k][3] for k in range(3)]
        for _ in range(y):
            p = [f32(p[k] + a[k][1]) for k in range(3)]
        if all(3 < p[k] < v.shape[k] - 4 for k in range(3)):
            vox.append(((x, y, z), np.array(p, dtype=np.float32)))

    def sinc(u):
        return 1.0 - abs(u) if abs(u) < 1e-7 else np.sin(np.pi * u) / (np.pi * u)

    def table(h, n=1201, win="hanning"):
        mid = (n - 1) // 2
        out = []
        for k in range(n):
            u = (k - mid) / mid * h
            w = 0.5 + 0.5 * np.cos(np.pi * u / h) if win == "hanning" else 1.0
            out.append(sinc(u) * w)
        return np.array(out)

    tables = {"h3": table(3.0), "h3.5": table(3.5)}

    def lookup(t, u, h, kind, dtype):
        mid = (len(t) - 1) // 2
        if kind == "a":
            x = dtype(u) / dtype(h) * dtype(mid) + dtype(mid)
        else:
            x = (dtype(u) + dtype(h)) / dtype(2 * h) * dtype(len(t) - 1)
        n = np.floor(x)
        i = int(n)
        if i < 0 or i + 1 >= len(t):
            return dtype(0)
        f = dtype(x - n)
        ti, tj = dtype(t[i]), dtype(t[i + 1])
        return dtype(ti + f * (tj - ti))

    variants = list(itertools.product(tables, "ab", (np.float32, np.float64), ("xyz", "zyx")))
    score = {k: 0 for k in variants}
    for (x, y, z), p in vox:
        target = want[x, y, z]
        base = np.floor(p).astype(int)
        for key in variants:
            tname, kind, dt, order = key
            h = 3.0 if tname == "h3" else 3.5
            t = tables[tname]
            k = [[lookup(t, f32(p[ax] - f32(base[ax] + d - 3)), h, kind, dt) for d in range(7)] for ax in range(3)]
            s, ws = dt(0), dt(0)
            for kz in range(7):
                for ky in range(7):
                    for kx in range(7):
                        val = v[base[0] + kx - 3, base[1] + ky - 3, base[2] + kz - 3]
                        if order == "xyz":
                            w = dt(dt(k[0][kx] * k[1][ky]) * k[2][kz])
                        else:
                            w = dt(dt(k[2][kz] * k[1][ky]) * k[0][kx])
                        s = dt(s + dt(w * dt(val)))
                        ws = dt(ws + w)
            got = f32(s / ws)
            score[key] += int(got == target)
    for k, n in sorted(score.items(), key=lambda kv: -kv[1]):
        print(n, "/", len(vox), k[0], k[1], k[2].__name__, k[3])


if __name__ == "__main__":
    main()
