# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""A numpy prototype of topup's estimation (one PE axis, no movements), from specs/topup.md and
black-box runs: the conventions are checked against FSL's debug files."""

import numpy as np
import scipy.sparse as sp
from scipy import ndimage as ndi


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


def ncoef(n, k):
    return n if k == 1 else int(np.ceil((n + 1) / k)) + 2


def offset(k):
    return 0 if k == 1 else -k


def basis(n, k, d=0, xs=None):
    xs = np.arange(n).astype(float) if xs is None else np.asarray(xs, float)
    nc = ncoef(n, k)
    return bspl((xs[:, None] - (offset(k) + np.arange(nc)[None, :] * k)) / k, d) / k**d


def apply(c, mats):
    out = c
    for ax in range(3):
        out = np.moveaxis(np.tensordot(mats[ax], np.moveaxis(out, ax, 0), axes=(1, 0)), 0, ax)
    return out


def kron3(mx, my, mz):
    return sp.kron(sp.kron(sp.csr_matrix(mz), sp.csr_matrix(my)), sp.csr_matrix(mx)).tocsr()


def kernel(sv):
    r = 2 * int(sv - 0.001) + 3
    j = np.arange(-r, r + 1)
    k = np.exp(-(j**2) / (2 * sv**2)).astype(np.float32).astype(np.float64)
    return k / k.sum()


class Images:
    """The scaled volumes, regridded once."""

    def __init__(self, vols, dx, smax, scale_each=True, regrid=True):
        self.dx = np.asarray(dx, float)
        self.n = np.array(vols[0].shape)
        m = [v.astype(np.float64).mean() for v in vols]
        g = np.mean(m)
        self.g = g
        self.scaled = [
            v.astype(np.float64) * (100.0 / (mi if scale_each else g)) for v, mi in zip(vols, m)
        ]
        if regrid:
            n2 = self.n + smax
            self.vr = ((self.n - 1) * self.dx - 1e-6) / (n2 - 1)
            coords = np.meshgrid(
                *[np.arange(n2[k]) * self.vr[k] / self.dx[k] for k in range(3)], indexing="ij"
            )
            self.reg = [
                ndi.map_coordinates(v, coords, order=3, mode="grid-wrap") for v in self.scaled
            ]
        else:
            self.vr = self.dx.copy()
            self.reg = self.scaled

    def level(self, s, fwhm):
        out = []
        for v in self.reg:
            n2 = np.array(v.shape)
            sub = (
                v.reshape(n2[0] // s, s, n2[1] // s, s, n2[2] // s, s).mean((1, 3, 5))
                if s > 1
                else v.copy()
            )
            vsub = self.vr * s
            if fwhm > 0:
                sig = fwhm / np.sqrt(8 * np.log(2))
                for ax in range(3):
                    sub = ndi.correlate1d(sub, kernel(sig / vsub[ax]), axis=ax, mode="wrap")
            out.append(ndi.spline_filter(sub, 3, mode="grid-wrap"))
        return out, self.vr * s


def interp(c, pos, deriv=None):
    """Cubic B-spline of coefficients c (periodic) at positions pos (3 arrays); value and the
    derivative along `deriv` (in c's voxels)."""
    sh = c.shape
    fl = [np.floor(p).astype(int) for p in pos]
    fr = [p - f for p, f in zip(pos, fl)]
    val = np.zeros(pos[0].shape)
    der = np.zeros(pos[0].shape) if deriv is not None else None
    wts = []
    for ax in range(3):
        w = [bspl(fr[ax] - o) for o in (-1, 0, 1, 2)]
        dw = [bspl(fr[ax] - o, 1) for o in (-1, 0, 1, 2)]
        wts.append((w, dw))
    for a in range(4):
        ia = (fl[0] + a - 1) % sh[0]
        for b in range(4):
            ib = (fl[1] + b - 1) % sh[1]
            for cc in range(4):
                ic = (fl[2] + cc - 1) % sh[2]
                v = c[ia, ib, ic]
                ww = wts[0][0][a] * wts[1][0][b] * wts[2][0][cc]
                val += ww * v
                if deriv is not None:
                    ws = [wts[0][0][a], wts[1][0][b], wts[2][0][cc]]
                    ws[deriv] = wts[deriv][1][(a, b, cc)[deriv]]
                    der += ws[0] * ws[1] * ws[2] * v
    return val, der


class Level:
    def __init__(self, imgs, s, fwhm, warpres, pe, t, lam, ssqlambda=True, regmod="bending"):
        self.coefs, self.vsub = imgs.level(s, fwhm)
        self.nt = imgs.n // s
        self.vt = imgs.dx * s
        self.k = [max(int(np.floor(warpres / self.vt[a] + 0.5)), 1) for a in range(3)]
        self.nc = [ncoef(self.nt[a], self.k[a]) for a in range(3)]
        self.B = [basis(self.nt[a], self.k[a]) for a in range(3)]
        self.dB = [basis(self.nt[a], self.k[a], 1) for a in range(3)]
        self.pe = pe  # list of per-volume p vectors
        self.t = t
        self.lam = lam
        self.ssq = ssqlambda
        self.e = [
            np.array([t[i] * pe[i][a] * imgs.dx[a] / self.vt[a] for a in range(3)])
            for i in range(len(pe))
        ]
        self.scale = self.vt / self.vsub
        nt = self.nt
        self.grid = np.meshgrid(*[np.arange(nt[a]).astype(float) for a in range(3)], indexing="ij")
        m = np.ones(tuple(nt))
        m[:, :, 0] = 0
        m[:, :, -1] = 0
        axes = {a for p in pe for a in range(2) if p[a] != 0}
        if axes == {1}:
            m[0] = 0
            m[-1] = 0
        elif axes == {0}:
            m[:, 0] = 0
            m[:, -1] = 0
        self.mask = m
        self.N = m.sum()
        self.R = self.bending()

    def bending(self):
        G = {}
        for a in range(3):
            n, k = self.nt[a], self.k[a]
            nc = self.nc[a]
            lo = offset(k) - 2 * k
            hi = offset(k) + (nc - 1) * k + 2 * k
            xs = np.arange(lo, hi + 1)
            for d in range(3):
                b = basis(n, k, d, xs) / self.vt[a] ** d
                G[(a, d)] = b.T @ b
        terms = [
            ((2, 0, 0), 1),
            ((0, 2, 0), 1),
            ((0, 0, 2), 1),
            ((1, 1, 0), 2),
            ((1, 0, 1), 2),
            ((0, 1, 1), 2),
        ]
        return sum(wt * kron3(G[(0, ds[0])], G[(1, ds[1])], G[(2, ds[2])]) for ds, wt in terms)

    def field(self, c, d=None):
        mats = [self.dB[a] if a == d else self.B[a] for a in range(3)]
        return apply(c.reshape(self.nc, order="F"), mats)

    def predict(self, c):
        h = self.field(c)
        hd = [self.field(c, a) for a in range(3)]
        us, J, fs, fd = [], [], [], []
        for i, cimg in enumerate(self.coefs):
            e = self.e[i]
            pos = [(self.grid[a] + h * e[a]) * self.scale[a] for a in range(3)]
            Ji = 1 + sum(e[a] * hd[a] for a in range(3))
            f, _ = interp(cimg, pos)
            dd = []
            for a in range(3):
                if e[a] != 0:
                    _, dv = interp(cimg, pos, a)
                    dd.append(dv * self.scale[a])
                else:
                    dd.append(None)
            us.append(Ji * f)
            J.append(Ji)
            fs.append(f)
            fd.append(dd)
        return h, us, J, fs, fd

    def cost(self, c, verbose=False):
        h, us, J, fs, fd = self.predict(c)
        M = len(us)
        ub = sum(us) / M
        ssd = sum((((ub - u) * self.mask) ** 2).sum() for u in us) / (self.N * (M - 1))
        E = float(c @ (self.R @ c))
        lamp = self.lam * ssd if self.ssq else self.lam
        if verbose:
            print(
                f"SSD = {ssd:.6g}\tn = {int(self.N)}\tReg = {lamp * E:.6g}\tCost = {ssd + lamp * E:.6g}"
            )
        return ssd + lamp * E, ssd, (h, us, J, fs, fd)

    def project(self, w, d):
        out = w
        for a in range(3):
            Bm = self.dB[a] if a == d else self.B[a]
            out = np.moveaxis(np.tensordot(Bm.T, np.moveaxis(out, a, 0), axes=(1, 0)), 0, a)
        return out.flatten(order="F")

    def grad(self, c, cache=None):
        cst, ssd, (h, us, J, fs, fd) = cache if cache is not None else self.cost(c)
        M = len(us)
        ub = sum(us) / M
        g = np.zeros(c.size)
        wa = np.zeros(tuple(self.nt))
        wb = [np.zeros(tuple(self.nt)) for _ in range(3)]
        for i in range(M):
            r = (ub - us[i]) * self.mask
            e = self.e[i]
            for a in range(3):
                if e[a] != 0:
                    wa += r * e[a] * J[i] * fd[i][a]
                    wb[a] += r * e[a] * fs[i]
        g = self.project(wa, None) + sum(self.project(wb[a], a) for a in range(3) if np.any(wb[a]))
        g *= -2 / (self.N * (M - 1))
        lamp = self.lam * ssd if self.ssq else self.lam
        return g + lamp * 2 * (self.R @ c), ssd

    def hessian(self, c, cache=None):
        cst, ssd, (h, us, J, fs, fd) = cache if cache is not None else self.cost(c)
        M = len(us)
        # per volume: alpha_i (B weight) and beta_i^a (d_a B weight)
        al = []
        be = []
        for i in range(M):
            e = self.e[i]
            al.append(sum(e[a] * J[i] * fd[i][a] for a in range(3) if e[a] != 0))
            be.append([e[a] * fs[i] if e[a] != 0 else np.zeros(tuple(self.nt)) for a in range(3)])
        ab = sum(al) / M
        bb = [sum(be[i][a] for i in range(M)) / M for a in range(3)]
        phis = [kron3(self.B[0], self.B[1], self.B[2])] + [
            kron3(*[self.dB[x] if x == a else self.B[x] for x in range(3)]) for a in range(3)
        ]
        comps = [[al[i] - ab] + [be[i][a] - bb[a] for a in range(3)] for i in range(M)]
        H = None
        for p in range(4):
            for q in range(4):
                w = sum(comps[i][p] * comps[i][q] for i in range(M)) * self.mask
                if not np.any(w):
                    continue
                term = phis[p].T @ sp.diags(w.flatten(order="F")) @ phis[q]
                H = term if H is None else H + term
        H = H * (2 / (self.N * (M - 1)))
        lamp = self.lam * ssd if self.ssq else self.lam
        return H + lamp * 2 * self.R


def pcg(A, b, tol=1e-3, maxit=500):
    x = np.zeros_like(b)
    r = b.copy()
    Minv = 1 / A.diagonal()
    z = Minv * r
    p = z.copy()
    rz = r @ z
    nb = np.linalg.norm(b)
    for it in range(maxit):
        Ap = A @ p
        al = rz / (p @ Ap)
        x += al * p
        r -= al * Ap
        if np.linalg.norm(r) / nb < tol:
            return x
        z = Minv * r
        rz2 = r @ z
        p = z + (rz2 / rz) * p
        rz = rz2
    return x


def lm(L, c, miter, verbose=True):
    lam_lm = 0.1
    cst, ssd, cache = L.cost(c, verbose)
    g, _ = L.grad(c, (cst, ssd, cache))
    H = L.hessian(c, (cst, ssd, cache))
    it = 0
    while it < miter:
        Hd = H + lam_lm * sp.diags(H.diagonal())
        step = pcg(Hd, -g)
        c2 = c + step
        cst2, ssd2, cache2 = L.cost(c2, verbose)
        if cst2 < cst:
            conv = 2 * abs(cst2 - cst) <= 1e-8 * (abs(cst2) + abs(cst) + 1e-10)
            c, cst, ssd, cache = c2, cst2, ssd2, cache2
            lam_lm /= 10
            it += 1
            if conv or it >= miter:
                break
            g, _ = L.grad(c, (cst, ssd, cache))
            H = L.hessian(c, (cst, ssd, cache))
        else:
            lam_lm *= 10
            if lam_lm > 1e20:
                break
    return c
