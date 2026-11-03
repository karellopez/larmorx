# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# FSL's rotation gradient at the start (debug/movements) with finite-difference image derivatives.
exec(open(__file__.replace("mov_fd_images.py", "mov_grad.py")).read().split("Yc = X")[0])
Yc = X - c[:, None, None, None]
fsl = np.array([-298.4080833, 54.36353386, 14.391763])
f1 = us[1]


def rot_g(d):
    A = np.array(
        [[-2 / N * (r1 * d[a] * Yc[b])[mask > 0].sum() for b in range(3)] for a in range(3)]
    )
    return np.array([A[2, 1] - A[1, 2], A[0, 2] - A[2, 0], A[1, 0] - A[0, 1]])


spl = grads[1]
cands = {
    "spline": spl,
    "central": [(np.roll(f1, -1, a) - np.roll(f1, 1, a)) / (2 * vt[a]) for a in range(3)],
    "forward": [(np.roll(f1, -1, a) - f1) / vt[a] for a in range(3)],
    "backward": [(f1 - np.roll(f1, 1, a)) / vt[a] for a in range(3)],
}
for k, d in cands.items():
    print(k, np.round(rot_g(d), 4), "diff", np.round(rot_g(d) - fsl, 4))
