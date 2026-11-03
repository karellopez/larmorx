# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# Rotation gradient at the start with other masks/weights (debug/movements).
exec(open(__file__.replace("mov_mask.py", "mov_grad.py")).read().split("Yc = X")[0])
Yc = X - c[:, None, None, None]
fsl = [13.5321484, 10.44574557, -298.4080833, 54.36353386, 14.391763]


def g_of(msk, Yc):
    out = []
    dP = [(-1, 0, 0), (0, 0, -1), (0, -Yc[2], Yc[1]), (Yc[2], 0, -Yc[0]), (-Yc[1], Yc[0], 0)]
    for v in dP:
        du = sum(grads[1][a] * v[a] for a in range(3))
        out.append(-2 / N * (r1 * du * msk).sum())
    return np.array(out)


full = np.ones(tuple(nt))
print("masked  ", g_of(mask, Yc) - fsl)
print("unmasked", g_of(full, Yc) - fsl)
m2 = np.ones(tuple(nt))
m2[:, :, 0] = 0
m2[:, :, -1] = 0
print("z only  ", g_of(m2, Yc) - fsl)
m3 = mask.copy()
m3[:, 0] = 0
m3[:, -1] = 0
print("+y edges", g_of(m3, Yc) - fsl)
J = np.stack([G[a] for a in range(3)])
cands = {
    "L1 j*vsub - c_img": J * vsub[:, None, None, None] - c[:, None, None, None],
    "L2 j*vt - (nt-1)vsub/2": X - ((nt - 1) * vsub / 2)[:, None, None, None],
    "L4 full": (J * 2 + 0.5) * dx[:, None, None, None] - ((I.n - 1) * dx / 2)[:, None, None, None],
    "L5 target": X - ((nt - 1) * vt / 2)[:, None, None, None],
    "L6 j*vt - n_img*vsub/2": X - (np.array(coefs[0].shape) * vsub / 2)[:, None, None, None],
    "L7 voxels (j - c)": J - ((nt - 1) / 2)[:, None, None, None],
}
for k, L in cands.items():
    print(k, np.round(g_of(mask, L) - fsl, 4))
