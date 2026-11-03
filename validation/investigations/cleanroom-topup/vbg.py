# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys

"""General VB model (black-box identification): per collection, mean-field over lines along the PE
axis, Gauss-Seidel in storage order; Q: 2D (vb2D, per slice and volume) or 3D stencil (vb3D per
volume; vb4D every volume, one phi/lambda)."""
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from vbm import *
import itertools, nibabel as nib


def resampling_matrix(h, pt):
    n = len(h)
    k = np.zeros((n, n))
    phi = lambda a: a + h[a % n] * pt
    for j in range(n):
        for u0, u1, a, b in [(j - 0.5, j, j - 1, j), (j, j + 0.5, j, j + 1)]:
            pa, pb = phi(a), phi(b)
            s = pb - pa
            v0 = pa + (u0 - a) * s
            v1 = pa + (u1 - a) * s
            lo, hi = min(v0, v1), max(v0, v1)
            sa = abs(s)
            if sa == 0:
                k[int(np.floor(lo + 0.5)) % n, j] += u1 - u0
                continue
            for i in range(int(np.floor(lo + 0.5)), int(np.floor(hi + 0.5)) + 1):
                ov = min(hi, i + 0.5) - max(lo, i - 0.5)
                if ov > 0:
                    k[i % n, j] += ov / sa
    return k


def field_k1(coef):
    """cubic B-spline with knot spacing 1, periodic along every axis, at the voxels."""
    from scipy import ndimage as nd

    v = coef.astype(float)
    for ax in range(3):
        v = nd.correlate1d(v, [1 / 6, 4 / 6, 1 / 6], axis=ax, mode="constant")
    return v


def lines_of(shape, pe):
    nd_ = len(shape)
    idx = np.arange(int(np.prod(shape))).reshape(shape, order="F")
    other = [a for a in range(nd_) if a != pe]
    out = []
    for c in itertools.product(*[range(shape[a]) for a in other][::-1]):
        c = c[::-1]
        sl = [None] * nd_
        for a, v in zip(other, c):
            sl[a] = v
        sl[pe] = slice(None)
        out.append(idx[tuple(sl)])
    return out


SKIP = None
VALIDCOUNT = False


def vb_block(ys, pts, h, shape, pe, Q, iters, count_mode="all", a0=1e-10, b0=1e10, z_extra=None):
    """ys: list of flat (x fastest) observed blocks; pts: displacement per Hz (voxels) per image; h flat field."""
    N = len(ys[0])
    lines = lines_of(shape, pe)
    n = shape[pe]
    if SKIP is not None:
        bad = SKIP(shape)
        lines = [L for L in lines if not bad[L].any()]
    Nv = sum(len(L) for L in lines) if VALIDCOUNT else N
    Kl = [[resampling_matrix(h[L], pt) for pt in pts] for L in lines]
    KtK = [sum(K.T @ K for K in ks) for ks in Kl]
    Kty = [sum(K.T @ y[L] for K, y in zip(ks, ys)) for ks, L in zip(Kl, lines)]
    cl = circ_line(n)
    f = np.zeros(N)
    Sig = [None] * len(lines)
    for i, L in enumerate(lines):
        Sig[i] = np.linalg.inv(KtK[i] + cl)
        f[L] = Sig[i] @ Kty[i]
    phi = lam = 1.0
    res = [(lam / phi, phi, lam)]
    nobs = {"all": len(ys) * Nv, "two": 2 * Nv}[count_mode]
    for it in range(iters):
        r2 = 0.0
        tr = 0.0
        for i, L in enumerate(lines):
            for K, y in zip(Kl[i], ys):
                r2 += np.sum((y[L] - K @ f[L]) ** 2)
            tr += np.trace(KtK[i] @ Sig[i])
        phi = (a0 + nobs // 2) / (1 / b0 + 0.5 * (r2 + tr))
        trq = sum(np.trace(Q[np.ix_(L, L)] @ Sig[i]) for i, L in enumerate(lines))
        lam = (a0 + Nv // 2) / (1 / b0 + 0.5 * (f @ Q @ f + trq))
        for i, L in enumerate(lines):
            rest = np.ones(N, bool)
            rest[L] = False
            Sig[i] = np.linalg.inv(phi * KtK[i] + lam * Q[np.ix_(L, L)])
            f[L] = Sig[i] @ (phi * Kty[i] - lam * Q[np.ix_(L, rest)] @ f[rest])
        res.append((lam / phi, phi, lam))
    return res, f


def effective(d, nimg):
    """FSL's interpolated images: y0 raw; y_a from eff<a> (2 out - y0)."""
    import os

    y0 = np.asarray(nib.load(f"{d}/p0.nii.gz").dataobj).astype(float)
    y0r = y0.copy()
    ys = [y0]
    if os.path.exists(f"{d}/eff0/hifi.nii.gz") and y0.ndim == 4:
        o = np.asarray(nib.load(f"{d}/eff0/hifi.nii.gz").dataobj).astype(float)
        y0[..., 1:] = o[..., 1:]
    for a in range(1, nimg):
        o = np.asarray(nib.load(f"{d}/eff{a}/hifi.nii.gz").dataobj).astype(float)
        ys.append(
            2 * o - (y0r[..., 0:1] if False else y0r)
            if o.ndim < 4
            else np.concatenate(
                [2 * o[..., :1] - y0r[..., :1], 2 * o[..., 1:] - (y0[..., 1:])], axis=3
            )
        )
    return ys
