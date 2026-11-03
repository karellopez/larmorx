# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""general jac prototype: python jacgen.py <case dir> [variant]"""

import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage

sys.path.insert(0, str(Path(__file__).parent))
from at_field import field


def rot(r):
    sx, cx = np.sin(r[0]), np.cos(r[0])
    sy, cy = np.sin(r[1]), np.cos(r[1])
    sz, cz = np.sin(r[2]), np.cos(r[2])
    Rx = np.array([[1, 0, 0], [0, cx, sx], [0, -sx, cx]])
    Ry = np.array([[cy, 0, -sy], [0, 1, 0], [sy, 0, cy]])
    Rz = np.array([[cz, sz, 0], [-sz, cz, 0], [0, 0, 1]])
    return Rx @ Ry @ Rz


def opts(args):
    o = {}
    it = iter(args)
    short = {
        "-i": "--imain",
        "-x": "--inindex",
        "-a": "--datain",
        "-t": "--topup",
        "-o": "--out",
        "-m": "--method",
        "-n": "--interp",
        "-d": "--datatype",
    }
    for a in it:
        if a in short:
            o[short[a]] = next(it)
        elif "=" in a:
            k, v = a.split("=", 1)
            o[k] = v
        else:
            o[a] = ""
    return o


def spline_deriv(img, axis, mode):
    """derivative along axis of the cubic spline of img (per voxel)"""
    co = ndimage.spline_filter(img, order=3, mode=mode)
    # derivative at integer points: (c[i+1] - c[i-1]) / 2
    if mode == "grid-constant":
        pad = [(0, 0)] * 3
        pad[axis] = (1, 1)
        cp = np.pad(co, pad)
        sl = lambda s: tuple(
            slice(s, s + img.shape[axis]) if a == axis else slice(None) for a in range(3)
        )
        return (cp[sl(2)] - cp[sl(0)]) / 2
    return (np.roll(co, -1, axis) - np.roll(co, 1, axis)) / 2


def run(d, variant=""):
    args = (d / "ARGS.txt").read_text().split()
    o = opts(args)
    names = o["--imain"].split(",")
    idx = [int(x) for x in o["--inindex"].split(",")]
    acq = np.loadtxt(d / o["--datain"], ndmin=2)
    tu = o["--topup"]
    linear = o.get("--interp") == "trilinear"
    imgs = [nib.load(str(d / n)) for n in names]
    n = np.array(imgs[0].shape[:3])
    vox = np.abs(np.asarray(imgs[0].header.get_zooms()[:3], float))
    coef = d / f"{tu}_fieldcoef.nii.gz"
    if coef.is_file():
        c = nib.load(str(coef))
        h = field(c)
        dh = [field(c, a) for a in range(3)]
        mv = d / f"{tu}_movpar.txt"
        mov = np.loadtxt(mv, ndmin=2) if mv.is_file() else None
        fieldimg = False
    else:
        fi = nib.load(str(d / f"{tu}.nii.gz"))
        h = np.asarray(fi.dataobj, np.float64)
        mov = None
        fieldimg = True
    hn = np.array(h.shape)
    if h.size == n.prod() and tuple(hn) != tuple(n):
        h = h.reshape(-1, order="F").reshape(tuple(n), order="F")
        if not fieldimg:
            dh = [x.reshape(-1, order="F").reshape(tuple(n), order="F") for x in dh]
    X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), -1)
    outs, valid = [], np.ones(n, bool)
    for k, (img, r) in enumerate(zip(imgs, idx)):
        data = np.asarray(img.dataobj, np.float64)
        vols = [data] if data.ndim == 3 else [data[..., v] for v in range(data.shape[3])]
        pe, t = acq[r - 1, :3], acq[r - 1, 3]
        if mov is not None:
            p = mov[r - 1]
            R = rot(p[3:])
            cen = (n - 1) * vox / 2
            q = (X * vox - cen - p[:3]) @ R + cen
            P0 = q / vox
        else:
            P0 = X.copy()
        P = P0 + (h * t)[..., None] * pe
        vk = np.ones(n, bool)
        for a in range(3):
            if pe[a] == 0:
                vk &= (P[..., a] >= 0) & (P[..., a] <= n[a] - 1)
        valid &= vk
        if not fieldimg:
            J = 1 + t * sum(pe[a] * dh[a] for a in range(3))
        else:
            J = np.ones(n)
            for a in range(3):
                if pe[a] != 0:
                    D = (
                        h * t * pe[a] * (vox[0] if variant == "fsl134" else vox[a])
                    )  # displacement mm
                    J = J + spline_deriv(D, a, "grid-constant") / vox[a]
        res = []
        for v, f in enumerate(vols):
            order = 1 if (linear or (k == 0 and v == 0 and variant != "fix132")) else 3
            res.append(
                ndimage.map_coordinates(f, np.moveaxis(P, -1, 0), order=order, mode="grid-wrap") * J
            )
        outs.append(np.stack(res, -1))
    g = np.mean(outs, axis=0)
    g = g[..., 0] if g.shape[-1] == 1 else g
    vm = valid if g.ndim == 3 else valid[..., None]
    return np.where(vm, g, 0)


if __name__ == "__main__":
    d = Path(sys.argv[1])
    variant = sys.argv[2] if len(sys.argv) > 2 else ""
    want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64)
    g = run(d, variant)
    e = np.abs(g - want) / (np.abs(want) + 1)
    zf, zg = (want == 0), (g == 0)
    print(
        d.name,
        variant,
        "rel(+1) p50 %.3g p99 %.3g max %.3g" % tuple(np.percentile(e, [50, 99, 100])),
        "zeros FSL",
        zf.sum(),
        "mine",
        zg.sum(),
        "differ",
        (zf != zg).sum(),
    )
