# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# Brute force: lever-arm coordinates, centres and derivative units for FSL's rotation gradient at the start.
exec(open(__file__.replace("mov_brute.py", "mov_grad.py")).read().split("Yc = X")[0])
import itertools

fsl = np.array([-298.4080833, 54.36353386, 14.391763])
Jg = np.stack([G[a] for a in range(3)])
nimg = np.array(coefs[0].shape)
gvox = [grads[1][a] * vsub[a] for a in range(3)]  # per image voxel
coords = {
    "mm(vt)": (Jg * vt[:, None, None, None], vt),
    "mm(vsub)": (Jg * vsub[:, None, None, None], vsub),
    "vox": (Jg.astype(float), np.ones(3)),
    "imgvox": (Jg * (vt / vsub)[:, None, None, None], np.ones(3)),
}
centres = {"c_img": lambda u: (nimg - 1) * u / 2 if False else None}
res = []
for (cn, (Xc, unit)), cen in itertools.product(
    coords.items(), ("img", "tgt", "full", "zero", "imgN", "tgtN")
):
    if cen == "img":
        c0 = (nimg - 1) / 2
    elif cen == "tgt":
        c0 = (nt - 1) / 2
    elif cen == "full":
        c0 = (I.n - 1) / 4
    elif cen == "imgN":
        c0 = nimg / 2
    elif cen == "tgtN":
        c0 = nt / 2.0
    else:
        c0 = np.zeros(3)
    if cn == "mm(vt)":
        cc = c0 * vt
    elif cn == "mm(vsub)":
        cc = c0 * vsub
    elif cn == "imgvox":
        cc = c0 * (vt / vsub) if cen.startswith("tgt") else c0
    else:
        cc = c0
    L = Xc - cc[:, None, None, None]
    for du in ("vsub", "vt", "vox"):
        d = [
            gvox[a] / (vsub[a] if du == "vsub" else vt[a] if du == "vt" else 1.0) for a in range(3)
        ]
        A = np.array(
            [[-2 / N * (r1 * d[a] * L[b])[mask > 0].sum() for b in range(3)] for a in range(3)]
        )
        g = np.array([A[2, 1] - A[1, 2], A[0, 2] - A[2, 0], A[1, 0] - A[0, 1]])
        res.append((np.abs(g - fsl).max(), cn, cen, du, np.round(g, 3)))
res.sort(key=lambda x: x[0])
for r in res[:6]:
    print(r)
