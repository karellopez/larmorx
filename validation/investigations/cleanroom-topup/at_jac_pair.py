# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np
from scipy import ndimage
sys.path.insert(0, str(Path(__file__).parent))
from at_field import field
def rot(r):
    sx, cx = np.sin(r[0]), np.cos(r[0]); sy, cy = np.sin(r[1]), np.cos(r[1]); sz, cz = np.sin(r[2]), np.cos(r[2])
    Rx = np.array([[1,0,0],[0,cx,sx],[0,-sx,cx]]); Ry = np.array([[cy,0,-sy],[0,1,0],[sy,0,cy]]); Rz = np.array([[cz,sz,0],[-sz,cz,0],[0,0,1]])
    return Rx @ Ry @ Rz
d = Path(sys.argv[1])
want = np.asarray(nib.load(str(d/"hifi.nii.gz")).dataobj, np.float64)
c = nib.load(str(d/"tu_fieldcoef.nii.gz"))
h = field(c); dh = [field(c, a) for a in range(3)]
mov = np.loadtxt(d/"tu_movpar.txt", ndmin=2)
acq = np.loadtxt(d/"acq.txt", ndmin=2)
img = nib.load(str(d/"half_dn.nii.gz")); f2 = np.asarray(img.dataobj, np.float64)
f1 = np.asarray(nib.load(str(d/"half_up.nii.gz")).dataobj, np.float64)
vox = np.asarray(img.header.get_zooms()[:3], float); n = np.array(f2.shape)
X = np.stack(np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij"), -1)
def predict(f, r, order, cmode, sign=1):
    pe, t = acq[r,:3], acq[r,3]
    p = mov[r]; R = rot(p[3:])
    cen = (n-1)*vox/2 if cmode == "c" else (n*vox/2 if cmode == "n" else np.zeros(3))
    xm = X*vox
    if sign == 1:
        q = (xm - cen - p[:3]) @ R + cen   # R^T (x - c - t) + c
    else:
        q = (xm - cen) @ R.T + cen + p[:3]  # R (x - c) + c + t
    P = q/vox + (h*t)[...,None]*pe
    valid = np.ones(n, bool)
    for a in range(3):
        if pe[a] == 0:
            valid &= (P[...,a] >= 0) & (P[...,a] <= n[a]-1)
    J = 1 + t*sum(pe[a]*dh[a] for a in range(3))
    return ndimage.map_coordinates(f, np.moveaxis(P,-1,0), order=order, mode="grid-wrap")*J, valid
u1, v1 = predict(f1, 0, 1, "c")
for cm in ("c", "n", "0"):
    for sign in (1, -1):
        u2, v2 = predict(f2, 1, 3, cm, sign)
        g = (u1+u2)/2
        m = want != 0
        e = np.abs(g-want)/(np.abs(want)+1)
        print(cm, sign, "rel p50 %.3g p99 %.3g" % tuple(np.percentile(e[m], [50, 99])), "FSL zeros", (~m).sum(), "invalid", (~(v1&v2)).sum(), "FSL zero & invalid", ((~m)&~(v1&v2)).sum())
