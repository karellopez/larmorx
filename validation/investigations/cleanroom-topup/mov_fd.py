# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# FSL's rotation gradient as a finite difference of the predicted image (debug/movements)?
exec(open(__file__.replace("mov_fd.py", "mov_grad_at.py")).read().split("for it in")[0])


def predict(i, h, Jf, p):
    e = acq[i, :3] * acq[i, 3] * dx
    R, _ = mats(p[3:])
    t = p[:3]
    Yc = X - c[:, None, None, None] - t[:, None, None, None]
    P = (
        np.einsum("ji,j...->i...", R, Yc)
        + c[:, None, None, None]
        + h[None] * e[:, None, None, None]
    )
    f, _ = tp.interp(coefs[i], [P[a] / vsub[a] for a in range(3)])
    return Jf[..., i] * f


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
    u0 = predict(0, h, Jf, mp[0])
    u1 = predict(1, h, Jf, mp[1])
    r1 = (u0 + u1) / 2 - u1
    out = {}
    for dl in (1e-6, 1e-3, 1e-2, 0.02, 0.05):
        g = []
        for k in (0, 2, 3, 4, 5):
            p = mp[1].copy()
            p[k] += dl
            du = (predict(1, h, Jf, p) - u1) / dl
            g.append(-2 / N * (r1 * du)[mask > 0].sum())
        out[dl] = np.array(g)
        print(it, dl, np.round(out[dl], 4), "ratio", np.round(gf / out[dl], 4))
    print(it, "fsl", gf)
