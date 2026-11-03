# SPDX-License-Identifier: Apache-2.0
"""Level 1 of debug/one-level at field 0: gradient, Hessian weights (ABC), first LM step."""

import sys

import nibabel as nib
import numpy as np
from scipy import ndimage as ndi

D = sys.argv[1]
img = nib.load(f"{D}/sub-002_dir-A_epi_average_merged_sliced_volreg.nii.gz")
a = np.asarray(img.dataobj, np.float32)
dx = np.array(img.header.get_zooms()[:3], np.float64)
n = np.array(a.shape[:3])
mask = np.asarray(nib.load(f"{D}/TopupDebugMasklevel01_iter00_attempt01.nii.gz").dataobj).astype(
    np.float64
)
g_fsl = np.loadtxt(f"{D}/TopupDebugGradient_level01_iter01_attempt00.txt")
abc = np.asarray(nib.load(f"{D}/TopupDebugABClevel01_iter01_attempt00.nii.gz").dataobj).astype(
    np.float64
)
smax = 2
s = 2
t = 0.0137951
p = [1.0, -1.0]
lam = 0.001
m = a.reshape(-1, a.shape[3]).astype(np.float64).mean(0)
nt = n // s
vs = ((n - 1) * dx - 1e-6) / (n + smax - 1)
vsub = vs * s
vt = s * dx


def prep(v, fwhm=6.0):
    vol = a[..., v].astype(np.float64) * (100.0 / m[v])
    n2 = n + smax
    coords = np.meshgrid(*[np.arange(n2[k]) * vs[k] / dx[k] for k in range(3)], indexing="ij")
    rg = ndi.map_coordinates(vol, coords, order=3, mode="grid-wrap")
    sub = rg.reshape(n2[0] // s, s, n2[1] // s, s, n2[2] // s, s).mean((1, 3, 5))
    sig = fwhm / np.sqrt(8 * np.log(2))
    for ax in range(3):
        sv = sig / vsub[ax]
        r = 2 * int(sv - 0.001) + 3
        j = np.arange(-r, r + 1)
        k = np.exp(-(j**2) / (2 * sv**2)).astype(np.float32).astype(np.float64)
        k /= k.sum()
        sub = ndi.correlate1d(sub, k, axis=ax, mode="wrap")
    return sub


def sample(sub, deriv_axis=None):
    c = ndi.spline_filter(sub, 3, mode="grid-wrap")
    sc = vt / vsub
    coords = np.meshgrid(*[np.arange(nt[k]) * sc[k] for k in range(3)], indexing="ij")
    if deriv_axis is None:
        return ndi.map_coordinates(c, coords, order=3, prefilter=False, mode="grid-wrap")
    e = 1e-4
    cp = [x.copy() for x in coords]
    cm = [x.copy() for x in coords]
    cp[deriv_axis] += e * sc[deriv_axis]
    cm[deriv_axis] -= e * sc[deriv_axis]
    return (
        ndi.map_coordinates(c, cp, order=3, prefilter=False, mode="grid-wrap")
        - ndi.map_coordinates(c, cm, order=3, prefilter=False, mode="grid-wrap")
    ) / (2 * e)


subs = [prep(v) for v in range(2)]
f = [sample(x) for x in subs]
fy = [sample(x, 1) for x in subs]  # per target voxel
ub = (f[0] + f[1]) / 2
N = mask.sum()
M = 2
e = [t * p[i] * dx[1] / vt[1] for i in range(2)]  # target voxels per Hz


def bspl(t, d=0):
    a_ = np.abs(t)
    s_ = np.sign(t)
    if d == 0:
        return np.where(a_ < 1, 2 / 3 - a_**2 + a_**3 / 2, np.where(a_ < 2, (2 - a_) ** 3 / 6, 0.0))
    if d == 1:
        return s_ * np.where(
            a_ < 1, -2 * a_ + 1.5 * a_**2, np.where(a_ < 2, -0.5 * (2 - a_) ** 2, 0.0)
        )
    return np.where(a_ < 1, -2 + 3 * a_, np.where(a_ < 2, (2 - a_), 0.0))


k = 2
nc = [int(np.ceil((nt[a_] + 1) / k)) + 2 for a_ in range(3)]


def Bm(ax, d=0, xs=None):
    xs = np.arange(nt[ax]).astype(float) if xs is None else xs
    return bspl((xs[:, None] - (np.arange(nc[ax])[None, :] - 1) * k) / k, d) / k**d


Bs = [Bm(ax) for ax in range(3)]
dBs = [Bm(ax, 1) for ax in range(3)]


def project(w, ax_deriv):
    out = w
    for ax in range(3):
        B = dBs[ax] if ax == ax_deriv else Bs[ax]
        out = np.moveaxis(np.tensordot(B.T, np.moveaxis(out, ax, 0), axes=(1, 0)), 0, ax)
    return out


wa = np.zeros(tuple(nt))
wb = np.zeros(tuple(nt))
al = [e[i] * fy[i] for i in range(2)]
be = [e[i] * f[i] for i in range(2)]
for i in range(2):
    r = (ub - f[i]) * mask
    wa += r * al[i]
    wb += r * be[i]
g = -(2 / (N * (M - 1))) * (project(wa, None) + project(wb, 1))
gv = g.flatten(order="F")
print(
    "gradient: max diff",
    np.abs(gv - g_fsl).max(),
    "max",
    np.abs(g_fsl).max(),
    "ratio",
    (gv @ g_fsl) / (g_fsl @ g_fsl),
)
# Hessian weights
ab = sum(al) / 2
bb = sum(be) / 2
wBB = sum((al[i] - ab) ** 2 for i in range(2)) * mask
wBy = sum((al[i] - ab) * (be[i] - bb) for i in range(2)) * mask
wyy = sum((be[i] - bb) ** 2 for i in range(2)) * mask
for nm, w, j in (("BB", wBB, 0), ("By", wBy, 2), ("yy", wyy, 5)):
    fslw = abc[..., j]
    sel = np.abs(fslw) > 1e-3 * np.abs(fslw).max()
    print(
        nm,
        "ratio fsl/mine median",
        np.median(fslw[sel] / w[sel]),
        "spread",
        np.percentile(fslw[sel] / w[sel], [1, 99]),
    )
np.save(f"{D}/../proto8_w.npy", np.stack([wBB, wBy, wyy]))
