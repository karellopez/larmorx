# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# The movement part of FSL's gradient at the start (debug/movements, iter01's gradient file).
import sys
from pathlib import Path
import numpy as np, nibabel as nib

sys.path.insert(0, str(Path(__file__).resolve().parent))
import tp

d = Path(sys.argv[1])
img = nib.load(str(d / "sub-002_dir-A_epi_average_merged_sliced_volreg.nii.gz"))
x = np.asarray(img.dataobj, np.float32)
vols = [x[..., i] for i in range(x.shape[3])]
dx = np.asarray(img.header.get_zooms()[:3], float)
I = tp.Images(vols, dx, 2, scale_each=False, regrid=True)
coefs, vsub = I.level(2, 6.0)
nt = I.n // 2
vt = dx * 2
G = np.meshgrid(*[np.arange(nt[a]).astype(float) for a in range(3)], indexing="ij")
X = np.stack([G[a] * vt[a] for a in range(3)])
c = (np.array(coefs[0].shape) - 1) * vsub / 2
mask = np.ones(tuple(nt))
mask[0] = 0
mask[-1] = 0
mask[:, :, 0] = 0
mask[:, :, -1] = 0
N = mask.sum()
m = 2
us, grads = [], []
for i in range(2):
    pos = [X[a] / vsub[a] for a in range(3)]
    f, _ = tp.interp(coefs[i], pos)
    g = [tp.interp(coefs[i], pos, a)[1] / vsub[a] for a in range(3)]
    us.append(f)
    grads.append(g)
ubar = (us[0] + us[1]) / 2
r1 = ubar - us[1]
print("SSD", ((ubar - us[0]) ** 2 + r1**2)[mask > 0].sum() / (N * (m - 1)))
Yc = X - c[:, None, None, None]
dP = {
    "tx": (-1, 0, 0),
    "ty": (0, -1, 0),
    "tz": (0, 0, -1),
    "rx": (0, -Yc[2], Yc[1]),
    "ry": (Yc[2], 0, -Yc[0]),
    "rz": (-Yc[1], Yc[0], 0),
}
for k, v in dP.items():
    du = sum(grads[1][a] * v[a] for a in range(3))
    gk = -2 / (N * (m - 1)) * (r1 * du)[mask > 0].sum()
    print(k, gk)


def rot(r):
    cx, sx = np.cos(r[0]), np.sin(r[0])
    cy, sy = np.cos(r[1]), np.sin(r[1])
    cz, sz = np.cos(r[2]), np.sin(r[2])
    Rx = np.array([[1, 0, 0], [0, cx, sx], [0, -sx, cx]])
    Ry = np.array([[cy, 0, -sy], [0, 1, 0], [sy, 0, cy]])
    Rz = np.array([[cz, sz, 0], [-sz, cz, 0], [0, 0, 1]])
    return Rx @ Ry @ Rz


def ssd(p):
    R = rot(p[3:])
    t = np.asarray(p[:3])
    P = (
        np.einsum("ij,j...->i...", R.T, Yc)
        + c[:, None, None, None]
        - (R.T @ t)[:, None, None, None]
    )
    f, _ = tp.interp(coefs[1], [P[a] / vsub[a] for a in range(3)])
    ub = (us[0] + f) / 2
    return (((ub - us[0]) ** 2 + (ub - f) ** 2)[mask > 0]).sum() / N


for k in (3, 4, 5):
    for h in (1e-6, 1e-4, 1e-3, 1e-2):
        p = np.zeros(6)
        p[k] = h
        q = np.zeros(6)
        q[k] = -h
        print(
            k, h, "central", (ssd(p) - ssd(q)) / (2 * h), "forward", (ssd(p) - ssd(np.zeros(6))) / h
        )
