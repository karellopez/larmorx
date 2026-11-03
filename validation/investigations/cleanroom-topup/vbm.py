# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys

"""Helpers: VB models of applytopup's vb methods (joint solves; small probes)."""
import numpy as np, nibabel as nib, re


def load(d, nimg):
    return [np.asarray(nib.load(f"{d}/p{i}.nii.gz").dataobj).astype(float) for i in range(nimg)]


def out(d):
    return np.asarray(nib.load(f"{d}/hifi.nii.gz").dataobj).astype(float)


def parse(path):
    res = []
    for l in open(path):
        if l.startswith("slice"):
            continue
        m = re.search(
            r"[Rr]elative lambda = ([-\d.e+]+),\s+phi = ([-\d.e+]+),\s+lambda = ([-\d.e+]+)", l
        )
        if m:
            res.append(tuple(map(float, m.groups())))
    return res


def circ_line(n):
    q = np.zeros((n, n))
    for i in range(n):
        q[i, i] += 6
        for d in (1, -1):
            q[i, (i + d) % n] += -4
        for d in (2, -2):
            q[i, (i + d) % n] += 1
    return q


def sopen(n):
    s = np.zeros((n, n))
    s[0, 0] = 1
    s[0, 1] = -1
    s[-1, -2] = -1
    s[-1, -1] = 1
    for i in range(1, n - 1):
        s[i, i - 1 : i + 2] = [-1, 2, -1]
    return s


def lap_sq(shape, pe_axis, w=None, periodic=None):
    """Q = D^T D, D the Laplacian on `shape` (C order arrays indexed [x,y,z] flattened x fastest),
    zero outside the image except along periodic axes; rows for every voxel within distance 1
    (rows centred outside the image too). w: per-axis weights of the second differences."""
    nd = len(shape)
    if w is None:
        w = [1.0] * nd
    if periodic is None:
        periodic = [a == pe_axis for a in range(nd)]
    N = int(np.prod(shape))

    def idx(p):
        q = list(p)
        for a in range(nd):
            if periodic[a]:
                q[a] %= shape[a]
            elif not (0 <= q[a] < shape[a]):
                return None
        # x fastest
        i = 0
        m = 1
        for a in range(nd):
            i += q[a] * m
            m *= shape[a]
        return i

    rows = []
    rng = [
        range(-0 if periodic[a] else -1, shape[a] + (0 if periodic[a] else 1)) for a in range(nd)
    ]
    import itertools

    for c in itertools.product(*rng):
        row = {}
        for a in range(nd):
            for d, v in ((-1, 1.0), (0, -2.0), (1, 1.0)):
                p = list(c)
                p[a] += d
                j = idx(p)
                if j is not None:
                    row[j] = row.get(j, 0.0) + w[a] * v
        rows.append(row)
    D = np.zeros((len(rows), N))
    for r, row in enumerate(rows):
        for j, v in row.items():
            D[r, j] = v
    return D.T @ D


def stencil_Q(shape, pe_axis, sten, periodic=None):
    """Q from a stencil dict {offset tuple: value}, truncated outside the image (periodic along periodic axes)."""
    nd = len(shape)
    if periodic is None:
        periodic = [a == pe_axis for a in range(nd)]
    N = int(np.prod(shape))
    Q = np.zeros((N, N))
    import itertools

    def lin(p):
        i = 0
        m = 1
        for a in range(nd):
            i += p[a] * m
            m *= shape[a]
        return i

    for c in itertools.product(*[range(s) for s in shape]):
        for off, v in sten.items():
            p = list(c)
            ok = True
            for a in range(nd):
                p[a] += off[a]
                if periodic[a]:
                    p[a] %= shape[a]
                elif not (0 <= p[a] < shape[a]):
                    ok = False
            if ok:
                Q[lin(c), lin(p)] += v
    return Q


def sten3(yz=1.0, z2=1.0, pe=1):
    """The 3D stencil (x, y, z offsets): Laplacian squared, with the (line axis, z) diagonals yz and z distance 2 z2."""
    st = {(0, 0, 0): 42.0}
    for a in range(3):
        for s in (1, -1):
            o = [0, 0, 0]
            o[a] = s
            st[tuple(o)] = -12.0
            o = [0, 0, 0]
            o[a] = 2 * s
            st[tuple(o)] = z2 if a == 2 else 1.0
    for a, b in [(0, 1), (0, 2), (1, 2)]:
        w = 2.0
        if {a, b} == {pe, 2}:
            w = 2.0 * yz / 2.0 if False else yz
        for s in (1, -1):
            for t in (1, -1):
                o = [0, 0, 0]
                o[a] = s
                o[b] = t
                st[tuple(o)] = w
    return st


def sten2():
    st = {(0, 0): 20.0}
    for a in range(2):
        for s in (1, -1):
            o = [0, 0]
            o[a] = s
            st[tuple(o)] = -8.0
            o = [0, 0]
            o[a] = 2 * s
            st[tuple(o)] = 1.0
    for s in (1, -1):
        for t in (1, -1):
            st[(s, t)] = 2.0
    return st


def gs_vb(ys, Ks, shape, pe, Q, iters, count=None, a0=1e-10, b0=1e10, trace=True):
    """Mean-field VB, lines along `pe` updated Gauss-Seidel in storage order; ys: list of flat
    observed vectors (x fastest); Ks: list of (N x N) resampling matrices (block diagonal per line)."""
    N = int(np.prod(shape))
    nd = len(shape)
    idx = np.arange(N).reshape(shape, order="F")
    other = [a for a in range(nd) if a != pe]
    lines = []
    import itertools

    for c in itertools.product(*[range(shape[a]) for a in other][::-1]):
        c = c[::-1]
        sl = [None] * nd
        for a, v in zip(other, c):
            sl[a] = v
        sl[pe] = slice(None)
        lines.append(idx[tuple(sl)])
    KtK = sum(K.T @ K for K in Ks)
    Kty = sum(K.T @ y for K, y in zip(Ks, ys))
    Q1 = np.zeros((N, N))
    cl = circ_line(shape[pe])
    for L in lines:
        Q1[np.ix_(L, L)] = cl
    Sig = np.zeros((N, N))
    f = np.zeros(N)
    for L in lines:
        Sl = np.linalg.inv(KtK[np.ix_(L, L)] + cl)
        Sig[np.ix_(L, L)] = Sl
        f[L] = Sl @ Kty[L]
    phi = lam = 1.0
    res = [(lam / phi, phi, lam)]
    nobs = count if count is not None else sum(len(y) for y in ys)
    for it in range(iters):
        r2 = sum(np.sum((y - K @ f) ** 2) for K, y in zip(Ks, ys))
        phi = (a0 + nobs // 2) / (1 / b0 + 0.5 * (r2 + np.trace(KtK @ Sig)))
        lam = (a0 + N // 2) / (1 / b0 + 0.5 * (f @ Q @ f + np.trace(Q @ Sig)))
        for L in lines:
            rest = np.ones(N, bool)
            rest[L] = False
            Sl = np.linalg.inv(phi * KtK[np.ix_(L, L)] + lam * Q[np.ix_(L, L)])
            f[L] = Sl @ (phi * Kty[L] - lam * Q[np.ix_(L, rest)] @ f[rest])
            Sig[np.ix_(L, L)] = Sl
        res.append((lam / phi, phi, lam))
    return res, f


def compare(res, fsl, f, o):
    rel = [max(abs(a - b) / max(abs(b), 1e-300) for a, b in zip(x, z)) for x, z in zip(res, fsl)]
    return "printed max rel %.2g first>1e-5 %s out rel %.2g" % (
        max(rel),
        next((i for i, v in enumerate(rel) if v > 1e-5), None),
        np.max(np.abs(f - o) / np.maximum(np.abs(o), 1e-30)),
    )


def y2_effective(S, nimg=2):
    imgs = load(S, nimg)
    sh = imgs[0].shape
    nx, ny, nz = sh[:3]
    lsr = out(S.rsplit("/", 2)[0] + "/lsr").reshape(nx, ny, nz, order="F")
    SS = sopen(ny).T @ sopen(ny)
    y2e = np.empty_like(imgs[1])
    for x in range(nx):
        for z in range(nz):
            y2e[x, :, z] = (2 * np.eye(ny) + 0.01 * SS) @ lsr[x, :, z] - imgs[0][x, :, z]
    return imgs[0], y2e


def sten_w(w, pe, half=False):
    """D = sum_d w_d Delta_d; the stencil of D^T D (len(w) dims); half: the (pe, z) diagonal at w_pe w_z."""
    nd_ = len(w)
    D = {}
    for a in range(nd_):
        for s, v in ((-1, 1.0), (0, -2.0), (1, 1.0)):
            o = [0] * nd_
            o[a] = s
            D[tuple(o)] = D.get(tuple(o), 0.0) + w[a] * v
    st = {}
    for o1, v1 in D.items():
        for o2, v2 in D.items():
            o = tuple(b - a for a, b in zip(o1, o2))
            st[o] = st.get(o, 0.0) + v1 * v2
    if half and nd_ == 3:
        for o in st:
            if (
                o[pe] != 0
                and o[2] != 0
                and abs(o[pe]) == 1
                and abs(o[2]) == 1
                and all(o[a] == 0 for a in range(3) if a not in (pe, 2))
            ):
                st[o] *= 0.5
    return {o: v for o, v in st.items() if v != 0}
