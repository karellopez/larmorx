# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §4.6 (spec writer, 2026-11-03): FSL's movement derivatives, from the spec's text.

FSL's derivative of a predicted volume with respect to a movement parameter is a forward
difference of the sampling positions, with steps of 1e-4 mm (translations) and 1e-5 rad
(rotations), the positions given by the movement matrices as FSL builds them (§4.3: each
elementary rotation from its angle rounded to single precision, with single-precision cosine and
sine and the axis entry (r / fl(|r|))^2). This script computes, on FSL's own debug files of the
replica suite's case `debug/movements` (one Levenberg-Marquardt level, subsampling 2, the
movements estimated: five parameters of the reversed scan), the movement part of the gradient:

- `fsl`:   the spec's forward difference (the reading FSL's `TopupDebugGradient` files hold);
- `exact`: the derivative of the cost (the movement matrices' exact derivatives);
- `--variant double-matrix`: the forward difference of matrices built in double precision (the
  rounding, not the step, is the cause: it gives the exact derivative to 1e-5 relative).

Usage: rotation_model.py <FSL run dir of debug/movements> [--variant double-matrix]
(`fsl_runs.py debug-movements <runs dir>` makes it, through the oracle cache).
The images, the regridding, the subsampling, the smoothing and the interpolation are those of
`validation/investigations/cleanroom-topup/tp.py` (written from the spec by the clean-room
implementer; checked there against FSL's debug files).
"""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "cleanroom-topup"))
import tp  # noqa: E402

F32 = np.float32


def elementary(axis: int, r: float, single: bool) -> np.ndarray:
    """The elementary rotation of §4.3 (spec writer, 2026-11-03): about `axis` by `r`."""
    R = np.eye(3)
    if single:
        th = float(F32(abs(r)))
        if th < 1e-8:
            return R
        q = r / th
        # single-precision cosine and sine of the rounded angle (correctly rounded here)
        c = float(F32(np.cos(th)))
        s = float(F32(np.sin(th))) * (1.0 if r >= 0 else -1.0)
        diag = q * q
    else:
        c, s, diag = np.cos(r), np.sin(r), 1.0
    i, j = [(1, 2), (0, 2), (0, 1)][axis]
    R[axis, axis] = diag
    if axis == 1:  # Ry = [[c, 0, -s], [0, 1, 0], [s, 0, c]]
        R[0, 0], R[0, 2], R[2, 0], R[2, 2] = c, -s, s, c
    else:  # Rx = [[1,0,0],[0,c,s],[0,-s,c]], Rz = [[c,s,0],[-s,c,0],[0,0,1]]
        R[i, i], R[i, j], R[j, i], R[j, j] = c, s, -s, c
    return R


def movement(p, centre, single=True) -> np.ndarray:
    """The 4 x 4 movement matrix of (tx, ty, tz, rx, ry, rz) about `centre` (§4.3)."""
    M = np.eye(4)
    for a in range(3):
        E = np.eye(4)
        E[:3, :3] = elementary(a, p[3 + a], single)
        E[:3, 3] = (np.eye(3) - E[:3, :3]) @ centre
        M = M @ E
    M[:3, 3] += p[:3]
    return M


def rot_exact(r):
    return elementary(0, r[0], False) @ elementary(1, r[1], False) @ elementary(2, r[2], False)


def main():
    d = Path(sys.argv[1])
    variant = sys.argv[sys.argv.index("--variant") + 1] if "--variant" in sys.argv else None
    img = nib.load(str(d / "sub-002_dir-A_epi_average_merged_sliced_volreg.nii.gz"))
    x = np.asarray(img.dataobj, np.float32)
    vols = [x[..., i] for i in range(x.shape[3])]
    if np.linalg.det(img.affine[:3, :3]) > 0:  # neurological storage: FSL holds it mirrored
        vols = [v[::-1] for v in vols]
        flip = True
    else:
        flip = False
    dx = np.asarray(img.header.get_zooms()[:3], float)
    acq = np.loadtxt(d / "sub-002_dir-A_epi_average_merged_sliced_volreg_encfile.txt")
    images = tp.Images(vols, dx, 2, scale_each=False, regrid=True)
    coefs, vsub = images.level(2, 6.0)
    nt = images.n // 2
    vt = dx * 2
    centre = (np.array(coefs[0].shape) - 1) * vsub / 2  # the level image's centre (§4.3)
    G = np.meshgrid(*[np.arange(nt[a]).astype(float) for a in range(3)], indexing="ij")
    X = np.stack([G[a] * vt[a] for a in range(3)])  # the target grid, mm
    Xh = np.concatenate([X, np.ones((1,) + X.shape[1:])])
    mask = np.ones(tuple(nt), bool)
    mask[0] = mask[-1] = False
    mask[:, :, 0] = mask[:, :, -1] = False
    n = int(mask.sum())
    m = 2
    names = ["tx", "tz", "rx", "ry", "rz"]  # the reversed scan's five parameters (§4.3)
    index = [0, 2, 3, 4, 5]
    worst = 0.0
    for it in ("01", "02", "03"):
        prev = f"{int(it) - 1:02d}"
        h = np.asarray(
            nib.load(str(d / f"TopupDebugFieldlevel01_iter{prev}_attempt01.nii.gz")).dataobj, float
        )
        J = np.asarray(
            nib.load(str(d / f"TopupDebugJacobianslevel01_iter{prev}_attempt01.nii.gz")).dataobj,
            float,
        )
        mp = np.loadtxt(d / f"TopupDebugMovementParameterslevel01_iter{prev}_attempt01.txt")
        if flip:
            h, J = h[::-1], J[::-1]
        u, deriv, pos = [], [], []
        for i in range(m):
            disp = np.stack([h * acq[i, a] * acq[i, 3] * dx[a] for a in range(3)])  # mm (§4.3)
            Minv = np.linalg.inv(movement(mp[i], centre))
            P = np.einsum("ij,j...->i...", Minv, Xh)[:3] + disp
            pv = [P[a] / vsub[a] for a in range(3)]
            f, _ = tp.interp(coefs[i], pv)
            g = [tp.interp(coefs[i], pv, a)[1] for a in range(3)]  # per image voxel
            u.append(J[..., i] * f)
            deriv.append(g)
            pos.append(Minv)
        ubar = sum(u) / m
        r = ubar - u[1]
        fsl, exact = [], []
        for name, k in zip(names, index):
            step = 1e-5 if k > 2 else 1e-4
            p2 = mp[1].copy()
            p2[k] += step
            single = variant != "double-matrix"
            M1 = np.linalg.inv(movement(mp[1], centre, single))
            M2 = np.linalg.inv(movement(p2, centre, single))
            dP = np.einsum("ij,j...->i...", M2 - M1, Xh)[:3] / float(F32(step))
            du = J[..., 1] * sum(deriv[1][a] * dP[a] / vsub[a] for a in range(3))
            fsl.append(-2.0 / (n * (m - 1)) * (r * du)[mask].sum())
            # exact: the derivative of R^T (y - c - t) + c
            R = rot_exact(mp[1][3:])
            Y = X - centre[:, None, None, None] - mp[1][:3, None, None, None]
            if k <= 2:
                dPe = -np.broadcast_to(R.T[:, k][:, None, None, None], X.shape)
            else:
                e = 1e-7
                q = mp[1][3:].copy()
                q[k - 3] += e
                q2 = mp[1][3:].copy()
                q2[k - 3] -= e
                dR = (rot_exact(q) - rot_exact(q2)) / (2 * e)
                dPe = np.einsum("ij,j...->i...", dR.T, Y)
            due = J[..., 1] * sum(deriv[1][a] * dPe[a] / vsub[a] for a in range(3))
            exact.append(-2.0 / (n * (m - 1)) * (r * due)[mask].sum())
        g = np.loadtxt(d / f"TopupDebugGradient_level01_iter{it}_attempt00.txt")[-5:]
        print(f"gradient {it} (at iter{prev}'s parameters)")
        for name, a, b, c in zip(names, g, fsl, exact):
            print(f"  {name}: FSL {a:12.6f}  model {b:12.6f}  exact {c:12.6f}  "
                  f"model/FSL-1 {b / a - 1:+.1e}  exact/FSL-1 {c / a - 1:+.2e}")
            worst = max(worst, abs(b / a - 1))
    print(f"largest relative difference model vs FSL: {worst:.2e}")


if __name__ == "__main__":
    main()
