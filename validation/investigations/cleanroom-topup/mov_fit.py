# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
exec(open(__file__.replace("mov_fit.py", "mov_grad.py")).read().split("def rot(r):")[0])
A = np.zeros((3, 3))
for a in range(3):
    for b in range(3):
        A[a, b] = -2 / N * (r1 * grads[1][a] * Yc[b])[mask > 0].sum()
B = np.array([-2 / N * (r1 * grads[1][a])[mask > 0].sum() for a in range(3)])  # sum w d_a u
print("A\n", A, "\nB", B)
fsl = np.array([-298.4080833, 54.36353386, 14.391763])
# g_rx = s_y A[2,1] - s_z A[1,2]; g_ry = s_z A[0,2] - s_x A[2,0]; g_rz = s_x A[1,0] - s_y A[0,1]
M = np.array([[0, A[2, 1], -A[1, 2]], [-A[2, 0], 0, A[0, 2]], [A[1, 0], -A[0, 1], 0]])
print("mine", M @ np.ones(3))
print("scales", np.linalg.solve(M, fsl))
# centre shift + : g_r(c+δ) = g_r(c) + δ × g_t, g_t = -B
gt = -B
# solve δ: δ × gt = fsl - mine  (least squares)
Cx = np.array(
    [[0, -gt[2], gt[1]], [gt[2], 0, -gt[0]], [-gt[1], gt[0], 0]]
)  # δ×gt = -gt×δ = -[gt]x δ
dlt, *_ = np.linalg.lstsq(-Cx, fsl - M @ np.ones(3), rcond=None)
print("centre shift", dlt, "residual", -Cx @ dlt - (fsl - M @ np.ones(3)))
import itertools

res = []
for sg in itertools.product((1, -1), repeat=3):
    for perm in itertools.permutations(range(3)):
        # lever arm L_b = sg_b * Yc_perm[b]
        Ap = np.zeros((3, 3))
        for a in range(3):
            for b in range(3):
                Ap[a, b] = sg[b] * A[a, perm[b]]
        g = np.array([Ap[2, 1] - Ap[1, 2], Ap[0, 2] - Ap[2, 0], Ap[1, 0] - Ap[0, 1]])
        res.append((np.abs(g - fsl).max(), sg, perm, g))
res.sort(key=lambda x: x[0])
for r in res[:4]:
    print(r)
