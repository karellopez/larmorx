# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# FSL's rotation derivative as a difference of trilinearly interpolated images (debug/movements, start)?
exec(open(__file__.replace("mov_trilinear.py", "mov_grad.py")).read().split("Yc = X")[0])
from scipy import ndimage as ndi

raw = []
for v in I.reg:
    n2 = np.array(v.shape)
    s = 2
    sub = v.reshape(n2[0] // s, s, n2[1] // s, s, n2[2] // s, s).mean((1, 3, 5))
    sig = 6.0 / np.sqrt(8 * np.log(2))
    for ax in range(3):
        sub = ndi.correlate1d(sub, tp.kernel(sig / vsub[ax]), axis=ax, mode="wrap")
    raw.append(sub)
fsl = np.array([-298.4080833, 54.36353386, 14.391763])
Yc = X - c[:, None, None, None]


def rot(r):
    cx, sx = np.cos(r[0]), np.sin(r[0])
    cy, sy = np.cos(r[1]), np.sin(r[1])
    cz, sz = np.cos(r[2]), np.sin(r[2])
    return (
        np.array([[1, 0, 0], [0, cx, sx], [0, -sx, cx]])
        @ np.array([[cy, 0, -sy], [0, 1, 0], [sy, 0, cy]])
        @ np.array([[cz, sz, 0], [-sz, cz, 0], [0, 0, 1]])
    )


def sample(img, r, order, spl):
    R = rot(r)
    P = np.einsum("ji,j...->i...", R, Yc) + c[:, None, None, None]
    pos = [P[a] / vsub[a] for a in range(3)]
    if order == 3:
        return tp.interp(spl, pos)[0]
    return ndi.map_coordinates(img, pos, order=1, mode="grid-wrap")


for order in (1, 3):
    for dl in (1e-4, 1e-3, 1e-2):
        g = []
        for k in range(3):
            e = np.zeros(3)
            e[k] = dl
            du = (sample(raw[1], e, order, coefs[1]) - sample(raw[1], -e, order, coefs[1])) / (
                2 * dl
            )
            g.append(-2 / N * (r1 * du)[mask > 0].sum())
        print("order", order, "central", dl, np.round(g, 4), "fsl", fsl)
