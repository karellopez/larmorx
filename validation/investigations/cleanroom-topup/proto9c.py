# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""First LM step of debug/one-level: H (masked or not) + lam' 2R, damping, PCG; vs FSL iter01."""

import sys

import nibabel as nib
import numpy as np
import scipy.sparse as sp

D = sys.argv[1]
mask = np.asarray(nib.load(f"{D}/TopupDebugMasklevel01_iter00_attempt01.nii.gz").dataobj).astype(
    np.float64
)
abc = np.asarray(nib.load(f"{D}/TopupDebugABClevel01_iter01_attempt00.nii.gz").dataobj).astype(
    np.float64
)
g = np.loadtxt(f"{D}/TopupDebugGradientWithReg_level01_iter01_attempt00.txt")
vox = np.array([6.0, 6.0, 6.5999985])
nt = (32, 32, 20)
k = 2
N = mask.sum()
M = 2
lam = 0.001
ssd0 = 191.809
TOL = float(sys.argv[2])


def bspl(t, d=0):
    a_ = np.abs(t)
    s_ = np.sign(t)
    if d == 0:
        return np.where(a_ < 1, 2 / 3 - a_**2 + a_**3 / 2, np.where(a_ < 2, (2 - a_) ** 3 / 6, 0.0))
    if d == 1:
        return s_ * np.where(
            a_ < 1, -2 * a_ + 1.5 * a_**2, np.where(a_ < 2, -0.5 * (2 - a_) ** 2, 0.0)
        )
    return np.where(a_ < 1, -2 + 3 * a_, np.where(a_ < 2, (2 - a_), 0.0))


nc = [int(np.ceil((nt[a_] + 1) / k)) + 2 for a_ in range(3)]


def Bm(ax, d=0, xs=None):
    xs = np.arange(nt[ax]).astype(float) if xs is None else xs
    return bspl((xs[:, None] - (np.arange(nc[ax])[None, :] - 1) * k) / k, d) / k**d


def kron3(mx, my, mz):  # F-order (x fastest): index = ix + nx*(iy + ny*iz)
    return sp.kron(sp.kron(sp.csr_matrix(mz), sp.csr_matrix(my)), sp.csr_matrix(mx)).tocsr()


Bx, By_, Bz = (Bm(ax) for ax in range(3))
dBy = Bm(1, 1)
Phi0 = kron3(Bx, By_, Bz)  # voxels x coefs
Phiy = kron3(Bx, dBy, Bz)


def H_gn(use_mask):
    w = abc * (mask[..., None] if use_mask else 1.0)
    W = lambda j: sp.diags(w[..., j].flatten(order="F"))
    H = Phi0.T @ W(0) @ Phi0 + Phi0.T @ W(2) @ Phiy + Phiy.T @ W(2) @ Phi0 + Phiy.T @ W(5) @ Phiy
    return (2 / (N * (M - 1))) * H


# R from integer sampling over the support, per mm, cross x2, / voxvol
def gram(ax, d):
    lo, hi = -3 * k, int(np.ceil((nt[ax] + 1) / k)) * k + 2 * k
    xs = np.arange(lo, hi + 1).astype(float)
    b = Bm(ax, d, xs) / vox[ax] ** d
    return b.T @ b


G = {(ax, d): gram(ax, d) for ax in range(3) for d in range(3)}
terms = [
    ((2, 0, 0), 1),
    ((0, 2, 0), 1),
    ((0, 0, 2), 1),
    ((1, 1, 0), 2),
    ((1, 0, 1), 2),
    ((0, 1, 1), 2),
]
R = sum(wt * kron3(G[(0, ds[0])], G[(1, ds[1])], G[(2, ds[2])]) for ds, wt in terms)


def pcg(A, b, tol=TOL, maxit=500):
    x = np.zeros_like(b)
    r = b.copy()
    Minv = 1 / A.diagonal()
    z = Minv * r
    pdir = z.copy()
    rz = r @ z
    nb = np.linalg.norm(b)
    for it in range(maxit):
        Ap = A @ pdir
        al = rz / (pdir @ Ap)
        x += al * pdir
        r -= al * Ap
        if np.linalg.norm(r) / nb < tol:
            return x, it + 1
        z = Minv * r
        rz2 = r @ z
        pdir = z + (rz2 / rz) * pdir
        rz = rz2
    return x, maxit


f1 = np.asarray(nib.load(f"{D}/TopupDebugFieldlevel01_iter01_attempt01.nii.gz").dataobj, np.float64)
for use_mask in (True, False):
    for regfac in (2.0,):
        H = H_gn(use_mask) + regfac * lam * ssd0 * R
        Hd = H + 0.1 * sp.diags(H.diagonal())
        dlt, its = pcg(Hd, -g)
        ex = np.linalg.solve(Hd.toarray(), -g)
        field = (Phi0 @ dlt).reshape(nt, order="F")
        fe = (Phi0 @ ex).reshape(nt, order="F")
        print(
            "mask",
            use_mask,
            "regfac",
            regfac,
            "pcg its",
            its,
            "max|field-fsl|",
            np.abs(field - f1).max(),
            "exact solve:",
            np.abs(fe - f1).max(),
            "max fsl",
            np.abs(f1).max(),
        )
