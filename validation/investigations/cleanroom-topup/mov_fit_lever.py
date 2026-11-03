# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# Fit a lever arm L_b = alpha_b (X_b - c_b - t_b) + beta_b to FSL's rotation gradients (3 points).
exec(open(__file__.replace("mov_fit_lever.py", "mov_grad_at.py")).read().split("for it in")[0])
rows, rhs = [], []
for it in ("01", "02", "03"):
    prev = f"{int(it) - 1:02d}"
    h = np.asarray(
        nib.load(str(d / f"TopupDebugFieldlevel01_iter{prev}_attempt01.nii.gz")).dataobj, float
    )
    Jf = np.asarray(
        nib.load(str(d / f"TopupDebugJacobianslevel01_iter{prev}_attempt01.nii.gz")).dataobj, float
    )
    mp = np.loadtxt(d / f"TopupDebugMovementParameterslevel01_iter{prev}_attempt01.txt")
    gf = np.loadtxt(d / f"TopupDebugGradient_level01_iter{it}_attempt00.txt")[-3:]
    us = []
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
        if i == 1:
            keep = (Jf[..., i], g, dR, Yc)
    r1 = sum(us) / 2 - us[1]
    J1, g1, dR, Yc = keep
    w = -2 / N * r1 * J1 * mask
    for k in range(3):
        row = []
        for b in range(3):  # alpha_b coefficient: sum_v w sum_a g_a dR[k][b, a] Yc_b
            row.append(sum((w * g1[a] * dR[k][b, a] * Yc[b]).sum() for a in range(3)))
        for b in range(3):  # beta_b
            row.append(sum((w * g1[a] * dR[k][b, a]).sum() for a in range(3)))
        rows.append(row)
        rhs.append(gf[k])
A = np.array(rows)
y = np.array(rhs)
print("exact (alpha=1, beta=0):", A @ np.r_[1, 1, 1, 0, 0, 0] - y)
sol, *_ = np.linalg.lstsq(A, y, rcond=None)
print("fit", sol, "residual", A @ sol - y)
sol2, *_ = np.linalg.lstsq(A[:, :3], y, rcond=None)
print("alpha only", sol2, "residual", A[:, :3] @ sol2 - y)
