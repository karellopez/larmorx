# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# Which movement model gives FSL's TopupDebugUnwarped at iter01 (debug/movements)?
import sys
from pathlib import Path
import numpy as np, nibabel as nib

sys.path.insert(0, str(Path(__file__).resolve().parent))
import tp

d = Path(sys.argv[1])
it = sys.argv[2] if len(sys.argv) > 2 else "01"
img = nib.load(str(d / "sub-002_dir-A_epi_average_merged_sliced_volreg.nii.gz"))
x = np.asarray(img.dataobj, np.float32)
vols = [x[..., i] for i in range(x.shape[3])]
dx = np.asarray(img.header.get_zooms()[:3], float)
# radiological? the stored axes after mirroring: determinant of affine negative -> FSL flips x
flip = np.linalg.det(img.affine[:3, :3]) > 0
print("neurological (flipped by FSL)?", flip)
if flip:
    vols = [v[::-1] for v in vols]
I = tp.Images(vols, dx, 2, scale_each=False, regrid=True)
coefs, vsub = I.level(2, 6.0)
nt = I.n // 2
vt = dx * 2
h = np.asarray(
    nib.load(str(d / f"TopupDebugFieldlevel01_iter{it}_attempt01.nii.gz")).dataobj, float
)
U = np.asarray(
    nib.load(str(d / f"TopupDebugUnwarpedlevel01_iter{it}_attempt01.nii.gz")).dataobj, float
)
Jf = np.asarray(
    nib.load(str(d / f"TopupDebugJacobianslevel01_iter{it}_attempt01.nii.gz")).dataobj, float
)
mp = np.loadtxt(d / f"TopupDebugMovementParameterslevel01_iter{it}_attempt01.txt")
if flip:
    h = h[::-1]
    U = U[::-1]
    Jf = Jf[::-1]
acq = np.loadtxt(d / "sub-002_dir-A_epi_average_merged_sliced_volreg_encfile.txt")


def rot(r):
    cx, sx = np.cos(r[0]), np.sin(r[0])
    cy, sy = np.cos(r[1]), np.sin(r[1])
    cz, sz = np.cos(r[2]), np.sin(r[2])
    Rx = np.array([[1, 0, 0], [0, cx, sx], [0, -sx, cx]])
    Ry = np.array([[cy, 0, -sy], [0, 1, 0], [sy, 0, cy]])
    Rz = np.array([[cz, sz, 0], [-sz, cz, 0], [0, 0, 1]])
    return Rx @ Ry @ Rz


G = np.meshgrid(*[np.arange(nt[a]).astype(float) for a in range(3)], indexing="ij")
X = np.stack([G[a] * vt[a] for a in range(3)])  # mm
i = 1
e = acq[i, :3] * acq[i, 3] * dx  # mm per Hz
R = rot(mp[i, 3:])
t = mp[i, :3]
cands = {}
for cname, c in {
    "target": (nt - 1) * vt / 2,
    "full": (I.n - 1) * dx / 2,
    "regsub": (np.array(coefs[0].shape) - 1) * vsub / 2,
}.items():
    for inv in (False, True):
        Rm = R.T if inv else R
        tm = -Rm @ t if inv else t

        def M(Y):
            Yc = Y - c[:, None, None, None]
            return (
                np.einsum("ij,j...->i...", Rm, Yc)
                + c[:, None, None, None]
                + tm[:, None, None, None]
            )

        for order in ("move-then-displace", "displace-then-move"):
            if order == "move-then-displace":
                P = M(X) + h[None] * e[:, None, None, None]
            else:
                P = M(X + h[None] * e[:, None, None, None])
            pos = [P[a] / vsub[a] for a in range(3)]
            f, _ = tp.interp(coefs[i], pos)
            dh = [tp.apply(np.zeros(1), []) if False else None for _ in range(3)]
            cands[(cname, inv, order)] = f
# Jacobian from FSL: u = J f
for k, f in cands.items():
    u = Jf[..., i] * f
    sl = (slice(2, -2), slice(2, -2), slice(2, -2))
    rel = np.abs(u - U[..., i])[sl].max() / np.abs(U[..., i]).max()
    print(k, f"max rel {rel:.3g}")
# and vol 0 (no movement)
pos0 = [(X[a] + h * acq[0, :3][a] * acq[0, 3] * dx[a]) / vsub[a] for a in range(3)]
f0, _ = tp.interp(coefs[0], pos0)
print("vol0", np.abs(Jf[..., 0] * f0 - U[..., 0]).max() / np.abs(U[..., 0]).max())
