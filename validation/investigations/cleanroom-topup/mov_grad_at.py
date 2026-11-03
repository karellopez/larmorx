# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# Movement gradient at FSL's later points (debug/movements): mine (exact) against FSL's gradient files.
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
acq = np.loadtxt(d / "sub-002_dir-A_epi_average_merged_sliced_volreg_encfile.txt")
mask = np.ones(tuple(nt))
mask[0] = 0
mask[-1] = 0
mask[:, :, 0] = 0
mask[:, :, -1] = 0
N = mask.sum()
m = 2


def mats(r):
    cx, sx = np.cos(r[0]), np.sin(r[0])
    cy, sy = np.cos(r[1]), np.sin(r[1])
    cz, sz = np.cos(r[2]), np.sin(r[2])
    Rx = np.array([[1, 0, 0], [0, cx, sx], [0, -sx, cx]])
    dRx = np.array([[0, 0, 0], [0, -sx, cx], [0, -cx, -sx]])
    Ry = np.array([[cy, 0, -sy], [0, 1, 0], [sy, 0, cy]])
    dRy = np.array([[-sy, 0, -cy], [0, 0, 0], [cy, 0, -sy]])
    Rz = np.array([[cz, sz, 0], [-sz, cz, 0], [0, 0, 1]])
    dRz = np.array([[-sz, cz, 0], [-cz, -sz, 0], [0, 0, 0]])
    R = Rx @ Ry @ Rz
    return R, [dRx @ Ry @ Rz, Rx @ dRy @ Rz, Rx @ Ry @ dRz]


for it in ("01", "02", "03"):
    prev = f"{int(it) - 1:02d}"
    h = np.asarray(
        nib.load(str(d / f"TopupDebugFieldlevel01_iter{prev}_attempt01.nii.gz")).dataobj, float
    )
    Jf = np.asarray(
        nib.load(str(d / f"TopupDebugJacobianslevel01_iter{prev}_attempt01.nii.gz")).dataobj, float
    )
    mp = np.loadtxt(d / f"TopupDebugMovementParameterslevel01_iter{prev}_attempt01.txt")
    gf = np.loadtxt(d / f"TopupDebugGradient_level01_iter{it}_attempt00.txt")[-5:]
    us, gr, Ps = [], [], []
    for i in range(2):
        e = acq[i, :3] * acq[i, 3] * dx
        R, dR = mats(mp[i, 3:])
        t = mp[i, :3]
        Yc = X - c[:, None, None, None] - t[:, None, None, None]
        P = (
            np.einsum("ji,j...->i...", R, Yc)
            + c[:, None, None, None]
            + h[None] * e[:, None, None, None]
        )
        pos = [P[a] / vsub[a] for a in range(3)]
        f, _ = tp.interp(coefs[i], pos)
        g = [tp.interp(coefs[i], pos, a)[1] / vsub[a] for a in range(3)]
        us.append(Jf[..., i] * f)
        dP = {"tx": -R.T[:, 0], "ty": -R.T[:, 1], "tz": -R.T[:, 2]}
        for k, nm in enumerate(("rx", "ry", "rz")):
            dP[nm] = np.einsum("ji,j...->i...", dR[k], Yc)
        gr.append((Jf[..., i], g, dP))
    ub = sum(us) / 2
    r1 = ub - us[1]
    J1, g1, dP1 = gr[1]
    mine = {}
    for k, v in dP1.items():
        if v.ndim == 1:
            du = J1 * sum(g1[a] * v[a] for a in range(3))
        else:
            du = J1 * sum(g1[a] * v[a] for a in range(3))
        mine[k] = -2 / N * (r1 * du)[mask > 0].sum()
    sel = [mine[k] for k in ("tx", "tz", "rx", "ry", "rz")]
    print(
        it,
        "mine",
        np.round(sel, 4),
        "\n   fsl ",
        np.round(gf, 4),
        "\n   ratio",
        np.round(gf / np.array(sel), 4),
    )
