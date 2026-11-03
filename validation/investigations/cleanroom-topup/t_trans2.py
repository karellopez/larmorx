# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import os
import sys

import nibabel as nib
import numpy as np
from scipy.optimize import minimize

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tp

D = sys.argv[1]
f1 = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel01_iter03_attempt01.nii.gz").dataobj, float)
f2 = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel02_iter00_attempt01.nii.gz").dataobj, float)
n1 = np.array(f1.shape)
n2 = np.array(f2.shape)
k1 = 2
c1 = tp.apply(f1, [np.linalg.pinv(tp.basis(n1[a], k1)) for a in range(3)])
P2 = [np.linalg.pinv(tp.basis(n2[a], 2)) for a in range(3)]
B2 = [tp.basis(n2[a], 2) for a in range(3)]


def model(params):
    pos = [params[2 * a] + np.arange(n2[a]) * params[2 * a + 1] for a in range(3)]
    fs = tp.apply(c1, [tp.basis(n1[a], k1, 0, pos[a]) for a in range(3)])
    return tp.apply(tp.apply(fs, P2), B2)


def obj(p):
    return np.sqrt(((model(p) - f2) ** 2).mean())


def refit_at(n, k, xs, lam):
    B = tp.basis(n, k, 0, xs)
    nc = B.shape[1]
    Dm = np.zeros((nc - 2, nc))
    for i in range(nc - 2):
        Dm[i, i : i + 3] = (1, -2, 1)
    return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)


for lam in (0.005, 0.05, 0.5):
    for off in (0.5, 0.0):
        mats = [refit_at(n2[a], 2, 2 * np.arange(n1[a]) + off, lam) for a in range(3)]
        c = tp.apply(f1, mats)
        f = tp.apply(c, B2)
        print(
            "old-voxel data at 2j+%.1f lam %g: max %.4g rms %.4g"
            % (off, lam, np.abs(f - f2).max(), np.sqrt(((f - f2) ** 2).mean()))
        )
from scipy import ndimage as ndi

for mode in ("grid-wrap", "mirror", "nearest", "reflect", "grid-constant"):
    for order in (3, 1):
        pos = np.meshgrid(*[-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)], indexing="ij")
        fsx = ndi.map_coordinates(f1, pos, order=order, mode=mode)
        for kp in (2, 4):
            mats = [np.linalg.pinv(tp.basis(n2[a], kp)) for a in range(3)]

            def rf(n, k, lam=0.005):
                B = tp.basis(n, k)
                nc = B.shape[1]
                Dm = np.zeros((nc - 2, nc))
                for i in range(nc - 2):
                    Dm[i, i : i + 3] = (1, -2, 1)
                return np.linalg.solve(B.T @ B + lam**2 * Dm.T @ Dm, B.T)

            c = tp.apply(fsx, [rf(n2[a], kp) for a in range(3)])
            f = tp.apply(c, [tp.basis(n2[a], kp) for a in range(3)])
            if kp != 2:
                f = tp.apply(tp.apply(f, [rf(n2[a], 2) for a in range(3)]), B2)
            print(
                "image interp",
                mode,
                "order",
                order,
                "kp",
                kp,
                "max %.4g rms %.4g" % (np.abs(f - f2).max(), np.sqrt(((f - f2) ** 2).mean())),
            )
pos = [-0.25 + np.arange(n2[a]) * 0.5 for a in range(3)]
fs = tp.apply(c1, [tp.basis(n1[a], k1, 0, pos[a]) for a in range(3)])
cm = tp.apply(fs, P2)
c2 = tp.apply(f2, P2)
r = c2 - cm
np.set_printoptions(precision=2, suppress=True, linewidth=250)
print("coef residual central x-line", r[:, 9, 6])
print("mine", cm[:, 9, 6])
print("fsl ", c2[:, 9, 6])
print("coef residual central y-line", r[9, :, 6])
print("coef residual central z-line", r[9, 9, :])
# FFT power along axes
for ax in range(3):
    R = np.abs(np.fft.rfft(r, axis=ax)) ** 2
    print(
        "axis", ax, "power by freq", np.round(R.mean(axis=tuple(a for a in range(3) if a != ax)), 3)
    )
for off in ():
    knots = [off + ((np.arange(B2[a].shape[1]) - 1) * 2) / 2 for a in range(3)]
    cq = tp.apply(c1, [tp.basis(n1[a], k1, 0, knots[a]) for a in range(3)])
    f = tp.apply(cq, B2)
    print(
        "coef = old field at knots, off",
        off,
        "max %.4g rms %.4g" % (np.abs(f - f2).max(), np.sqrt(((f - f2) ** 2).mean())),
    )
# what does f2 look like when its coefficients are compared with old field at knots
c2 = tp.apply(f2, P2)
knots = [-0.25 + ((np.arange(B2[a].shape[1]) - 1) * 2) / 2 for a in range(3)]
fk = tp.apply(c1, [tp.basis(n1[a], k1, 0, knots[a]) for a in range(3)])
print(
    "c2 vs old field at knots: rms", np.sqrt(((c2 - fk) ** 2).mean()), "c2 vs refit of sampled: rms"
)
import sys as _s

_s.exit()
p0 = np.array([-0.25, 0.5] * 3)
print("start", obj(p0))
r = minimize(obj, p0, method="Nelder-Mead", options={"maxiter": 4000, "xatol": 1e-6, "fatol": 1e-9})
print(r.x, r.fun)
