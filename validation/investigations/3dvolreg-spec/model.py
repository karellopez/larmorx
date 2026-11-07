# SPDX-License-Identifier: Apache-2.0
"""A model of AFNI 3dvolreg written from ``specs/3dvolreg.md`` (and black-box runs only).

It implements the spec's statements one by one, in numpy, so that the spec can be checked
against AFNI's binary (``check.py``): the conventions (§4), the four-shear resampling with every
interpolation method (§5), the blurs (§6, FIR only), the basis and the iterations (§7), the
program's flow with ``-twopass`` and its coarse searches (§8), and the text outputs (§10.2).
Not modelled: ``-tshift``, ``-rotparent``/``-gridparent``, ``-savedisp``, the FFT blur (``-twoblur``
above 14), the messages. Its automasks are larmorx's clean-room ``lx.afni.automask``
(``implementation="original"``), so the model needs no restricted material at all; a clean-room
implementer may read and run it.

Arrays are indexed ``v[k, j, i]`` (C order), so a flat index is ``i + nx*j + nx*ny*k``: AFNI's
storage order. AFNI's axis ``a`` (0 = i, 1 = j, 2 = k) is numpy axis ``2 - a``.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

import nibabel as nib
import numpy as np
from scipy import ndimage

F32 = np.float32
F64 = np.float64
PI = math.pi

# ------------------------------------------------------------------------------------------
# §4.1 AFNI's grid

#: Orientation names; the DICOM coordinate each runs along and its sign.
ORIENT = {
    "R2L": (0, +1),
    "L2R": (0, -1),
    "P2A": (1, -1),
    "A2P": (1, +1),
    "I2S": (2, +1),
    "S2I": (2, -1),
}
FROM = {"R2L": "R", "L2R": "L", "P2A": "P", "A2P": "A", "I2S": "I", "S2I": "S"}
TO = {"R2L": "L", "L2R": "R", "P2A": "A", "A2P": "P", "I2S": "S", "S2I": "I"}


@dataclass
class Grid:
    n: tuple[int, int, int]  # (nx, ny, nz)
    orient: tuple[str, str, str]
    step: np.ndarray  # signed, float32, per axis
    org: np.ndarray  # float32, per axis

    @property
    def delta(self) -> np.ndarray:  # |step|, 1 where 0
        d = np.abs(self.step).astype(F32)
        return np.where(d == 0, F32(1), d).astype(F32)

    def code(self, letter: str) -> int:
        for n, o in enumerate(self.orient):
            if FROM[o] == letter:
                return n + 1
            if TO[o] == letter:
                return -(n + 1)
        return -99

    def handedness(self) -> int:
        m = np.zeros((3, 3))
        for n, o in enumerate(self.orient):
            c, s = ORIENT[o]
            m[c, n] = s
        return 1 if np.linalg.det(m) > 0 else -1

    def P(self) -> np.ndarray:
        """§4.1: dataset axes to DICOM order."""
        p = np.zeros((3, 3))
        for n, o in enumerate(self.orient):
            c, s = ORIENT[o]
            p[c, n] = s
        return p

    def to_dicom(self, u):
        out = [F32(0)] * 3
        for n, o in enumerate(self.orient):
            c, _ = ORIENT[o]
            out[c] = F32(u[n])  # a permutation: the coordinates are already DICOM-valued
        return np.array(out, F32)

    def centre(self) -> np.ndarray:
        """THE centre of §4.1: 0.5f*(first + last), DICOM order, float32."""
        a = self.org.astype(F32)
        b = np.array(
            [F32(self.org[n] + F32(F32(self.n[n] - 1) * self.step[n])) for n in range(3)], F32
        )
        a, b = self.to_dicom(a), self.to_dicom(b)
        return (F32(0.5) * (a + b)).astype(F32)

    def axis_centre(self) -> np.ndarray:
        return np.array(
            [F32(F64(self.org[n]) + 0.5 * (self.n[n] - 1) * F64(self.step[n])) for n in range(3)],
            F32,
        )


def read_grid(img) -> Grid:
    """§4.1 from a nibabel image (sform if its code > 0, else qform)."""
    h = img.header
    scode = int(h["sform_code"])
    m = img.get_sform() if scode > 0 else img.get_qform()
    m = np.asarray(m, F64)
    ornt = nib.orientations.io_orientation(m)  # per voxel axis: world axis (RAS) and sign
    names = []
    for ax, sg in ornt:
        ax = int(ax)
        if ax == 0:
            names.append("L2R" if sg > 0 else "R2L")
        elif ax == 1:
            names.append("P2A" if sg > 0 else "A2P")
        else:
            names.append("I2S" if sg > 0 else "S2I")
    if scode > 0:
        size = [F32(math.sqrt(m[0, n] ** 2 + m[1, n] ** 2 + m[2, n] ** 2)) for n in range(3)]
    else:
        size = [F32(h["pixdim"][n + 1]) for n in range(3)]
    step, org = [], []
    for n, o in enumerate(names):
        c, s = ORIENT[o]
        step.append(F32(size[n] if s > 0 else -size[n]))
        org.append(F32(m[2, 3] if c == 2 else -m[c, 3]))
    return Grid(
        tuple(int(x) for x in img.shape[:3]), tuple(names), np.array(step, F32), np.array(org, F32)
    )


def read_volumes(img) -> tuple[list[np.ndarray], str]:
    """The volumes scaled to float32 (§8.2, 3dTshift §6's table) as v[k, j, i], and the output's
    storage kind (float when a brick factor or a float conversion is involved)."""
    raw = np.asanyarray(img.dataobj.get_unscaled())
    # nibabel moves the header's scaling into the array proxy (1 and 0 when there is none)
    slope, inter = float(img.dataobj.slope), float(img.dataobj.inter)
    slope = slope if np.isfinite(slope) else 0.0
    inter = inter if np.isfinite(inter) else 0.0
    if raw.ndim == 3:
        raw = raw[..., None]
    dt = raw.dtype
    factor = None
    if dt in (np.dtype("uint8"), np.dtype("int16"), np.dtype("float32")) and not (
        slope != 0 and inter != 0
    ):
        kind = {np.dtype("uint8"): "byte", np.dtype("int16"): "short"}.get(dt, "float")
        if slope not in (0.0, 1.0):
            factor = F32(slope)
            kind = "float"
        convert = False
    else:
        kind, convert = "float", True
    vols = []
    for t in range(raw.shape[3]):
        v = np.ascontiguousarray(raw[..., t].transpose(2, 1, 0)).astype(F32)
        if convert and slope != 0:
            v = (v.astype(F64) * F64(F32(slope)) + F64(F32(inter))).astype(F32)
        if factor is not None:
            v = (factor * v).astype(F32)
        vols.append(v)
    return vols, kind


def store(v: np.ndarray, kind: str) -> np.ndarray:
    """§10.1's storage conversion (``reproduce``: AFNI-87's stretch for bytes)."""
    if kind == "short":
        return np.rint(np.clip(v, -32767, 32767)).astype(np.int16)
    if kind == "byte":
        lo, hi = float(v.min()), float(v.max())
        hi = lo + 1 if hi <= lo else hi
        sc = 255.9 / (hi - lo)
        return np.trunc(sc * (v - F32(lo)).astype(F64)).astype(np.uint8)
    return v


# ------------------------------------------------------------------------------------------
# §4.3 rotations


def E(a: int, th: float) -> np.ndarray:
    c, s = math.cos(th), math.sin(th)
    if a == 0:
        return np.array([[1, 0, 0], [0, c, s], [0, -s, c]], F64)
    if a == 1:
        return np.array([[c, 0, -s], [0, 1, 0], [s, 0, c]], F64)
    return np.array([[c, s, 0], [-s, c, 0], [0, 0, 1]], F64)


def mul(A, B):
    C = np.zeros((3, 3))
    for i in range(3):
        for j in range(3):
            C[i, j] = A[i, 0] * B[0, j] + A[i, 1] * B[1, j] + A[i, 2] * B[2, j]
    return C


def matvec(A, x):
    return np.array([A[i, 0] * x[0] + A[i, 1] * x[1] + A[i, 2] * x[2] for i in range(3)], F64)


def R3(a1, t1, a2, t2, a3, t3):
    return mul(E(a3, t3), mul(E(a2, t2), E(a1, t1)))


# ------------------------------------------------------------------------------------------
# §5 the resampler


@dataclass
class Shear:
    axis: int
    coef: tuple[float, float, float]  # coefficient per original axis (own axis unused)
    shift: float


def _decompose(q, e):
    """§5.3 step 2 in the permuted frame; None when it fails."""
    D = np.linalg.det(q)
    q10, q11, q20, q21 = q[1, 0], q[1, 1], q[2, 0], q[2, 1]
    den = q11 * q20 - q10 * q21
    if D == 0 or q10 == 0 or den == 0:
        return None
    f = float(np.cbrt(D))
    ay = q10
    b1 = (q11 - f) / q10
    az = den / f
    bz = (q20 - az) / q10
    if az == 0:
        return None
    c1 = (q[2, 2] - f - bz * q[1, 2]) / az
    cy = q[1, 2] - q10 * c1
    A = np.array([[q10, q20], [q11, q21]])
    b2, c2 = np.linalg.solve(A, np.array([q[0, 0] - f, q[0, 1] - f * b1]))
    sy = e[1]
    sz = e[2] - bz * e[1]
    sx = e[0] - b2 * e[1] - c2 * e[2]
    norm = max(abs(b1), abs(c1), abs(ay), abs(cy), abs(az), abs(bz))
    # in the permuted frame: X1 (axis 0), Y (1), Z (2), X2 (0)
    sh = [
        (0, (0.0, b1, c1), 0.0),
        (1, (ay, 0.0, cy), sy),
        (2, (az, bz, 0.0), sz),
        (0, (0.0, b2, c2), sx),
    ]
    return norm, sh


ORDERS = [(0, 1, 2), (0, 2, 1), (1, 0, 2), (1, 2, 0), (2, 0, 1), (2, 1, 0)]


def shear_best(Q, d):
    tr = Q[0, 0] + Q[1, 1] + Q[2, 2]
    es = abs(Q[0, 1]) + abs(Q[0, 2]) + abs(Q[1, 0]) + abs(Q[1, 2]) + abs(Q[2, 0]) + abs(Q[2, 1])
    if tr >= 2.99999 and es / tr < 1e-6:
        return [
            Shear(0, (0, 0, 0), 0.0),
            Shear(1, (0, 0, 0), d[1]),
            Shear(2, (0, 0, 0), d[2]),
            Shear(0, (0, 0, 0), d[0]),
        ]
    best, bestn = None, 1e38
    for o in ORDERS:
        q = np.array([[Q[o[m], o[n]] for n in range(3)] for m in range(3)])
        e = np.array([d[o[m]] for m in range(3)])
        r = _decompose(q, e)
        if r is None:
            continue
        norm, sh = r
        if norm < bestn:
            out = []
            for ax, coef, s in sh:
                c = [0.0, 0.0, 0.0]
                for n in range(3):
                    c[o[n]] = coef[n]
                out.append(Shear(o[ax], tuple(c), s))
            best, bestn = out, norm
    return best


def shears_for(Q, d, delta):
    """§5.2 flip, §4.2 voxel scaling, §5.3 shears. Returns (flip axes or None, shears)."""
    Q = Q.copy()
    flip = None
    if Q[0, 0] + Q[1, 1] + Q[2, 2] < 1.0:
        dg = [Q[0, 0], Q[1, 1], Q[2, 2]]
        t = 0
        if dg[0] < dg[1]:
            t = 1
        if dg[t] < dg[2]:
            t = 2
        p, q = [x for x in range(3) if x != t]
        if Q[p, p] + Q[q, q] < -0.02:
            F = np.eye(3)
            F[p, p] = F[q, q] = -1
            Q = mul(Q, F)
            flip = (p, q)
    dl = [F64(x) for x in delta]
    dv = np.array([d[n] / dl[n] for n in range(3)])
    for m in range(3):
        for n in range(3):
            if m != n:
                Q[m, n] *= dl[n] / dl[m]
    sh = shear_best(Q, dv)
    if sh is None:
        P = R3(0, 1.09e-6, 1, 1.22e-6, 2, 1.37e-6)
        Q = mul(P.T, mul(Q, P))
        sh = shear_best(Q, dv)
    return flip, sh


def _floor_ia(beta: np.ndarray) -> np.ndarray:
    ia = np.trunc(beta).astype(np.int64)
    return ia - (beta < 0)


def _weights(method, x):
    """§5.6's weights (float32), x the float32 fractions."""
    X = x.astype(F64)
    XX = (x * x).astype(F64)
    if method == "linear":
        return [(1.0 - X).astype(F32), x.astype(F32)], [0, 1]
    if method == "cubic":
        w = [
            X * (1.0 - X) * (X - 2.0) * 0.1666667,
            (X + 1.0) * (X - 1.0) * (X - 2.0) * 0.5,
            X * (X + 1.0) * (2.0 - X) * 0.5,
            X * (X + 1.0) * (X - 1.0) * 0.1666667,
        ]
        return [v.astype(F32) for v in w], [-1, 0, 1, 2]
    if method == "quintic":
        w = [
            X * (XX - 1.0) * (2.0 - X) * (X - 3.0) * 0.008333333,
            X * (XX - 4.0) * (X - 1.0) * (X - 3.0) * 0.041666667,
            (XX - 4.0) * (XX - 1.0) * (3.0 - X) * 0.083333333,
            X * (XX - 4.0) * (X + 1.0) * (X - 3.0) * 0.083333333,
            X * (XX - 1.0) * (X + 2.0) * (3.0 - X) * 0.041666667,
            X * (XX - 1.0) * (XX - 4.0) * 0.008333333,
        ]
        return [v.astype(F32) for v in w], [-2, -1, 0, 1, 2, 3]
    if method == "heptic":
        w = [
            X * (XX - 1.0) * (XX - 4.0) * (X - 3.0) * (4.0 - X) * 0.0001984126984,
            X * (XX - 1.0) * (X - 2.0) * (XX - 9.0) * (X - 4.0) * 0.001388888889,
            X * (X - 1.0) * (XX - 4.0) * (XX - 9.0) * (4.0 - X) * 0.004166666667,
            (XX - 1.0) * (XX - 4.0) * (XX - 9.0) * (X - 4.0) * 0.006944444444,
            X * (X + 1.0) * (XX - 4.0) * (XX - 9.0) * (4.0 - X) * 0.006944444444,
            X * (XX - 1.0) * (X + 2.0) * (XX - 9.0) * (X - 4.0) * 0.004166666667,
            X * (XX - 1.0) * (XX - 4.0) * (X + 3.0) * (4.0 - X) * 0.001388888889,
            X * (XX - 1.0) * (XX - 4.0) * (XX - 9.0) * 0.0001984126984,
        ]
        return [v.astype(F32) for v in w], [-3, -2, -1, 0, 1, 2, 3, 4]
    raise ValueError(method)


#: The order of the terms in the sums (§5.6).
SUM_ORDER = {
    "linear": [0, 1],
    "cubic": [-1, 0, 1, 2],
    "quintic": [-2, -1, 0, 1, 2, 3],
    "heptic": [-2, -1, 0, 1, 2, 3, -3, 4],
}


def shift_rows_poly(rows: np.ndarray, alpha: np.ndarray, method: str) -> np.ndarray:
    """§5.6 for NN and the polynomial methods; rows (m, n) float32, alpha (m,) float32."""
    n = rows.shape[1]
    beta = (-alpha).astype(F32)
    ia = _floor_ia(beta.astype(F64))
    x = (beta - ia.astype(F32)).astype(F32)
    out = np.zeros_like(rows)
    ok = (ia > -n) & (ia < n)
    i = np.arange(n)[None, :]
    base = i + ia[:, None]
    if method == "nn":
        idx = base
        valid = (idx >= 0) & (idx < n)
        g = np.take_along_axis(rows, np.clip(idx, 0, n - 1), axis=1)
        out = np.where(valid & ok[:, None], g, F32(0)).astype(F32)
        return out
    w, ks = _weights(method, x)
    wk = dict(zip(ks, w, strict=True))
    kmin, kmax = min(ks), max(ks)
    interior = (base + kmin >= 0) & (base + kmax <= n - 1)
    samp = {}
    for k in ks:
        idx = base + k
        valid = (idx >= 0) & (idx < n)
        samp[k] = np.where(valid, np.take_along_axis(rows, np.clip(idx, 0, n - 1), axis=1), F32(0))
    s32 = None
    s64 = None
    for k in SUM_ORDER[method]:
        p32 = (wk[k][:, None] * samp[k]).astype(F32)
        p64 = wk[k][:, None].astype(F64) * samp[k].astype(F64)
        s32 = p32 if s32 is None else (s32 + p32).astype(F32)
        s64 = p64 if s64 is None else s64 + p64
    res = np.where(interior, s32, s64.astype(F32)).astype(F32)
    return np.where(ok[:, None], res, F32(0)).astype(F32)


def _length(nst: int) -> int:
    L = max(nst, 2)
    while True:
        x = L
        if x % 2 == 0:
            while x % 2 == 0:
                x //= 2
            if x in (1, 3, 5, 15):
                return L
        L += 1


def fourier_pairs(f: np.ndarray, g: np.ndarray, af: np.ndarray, ag: np.ndarray, L: int, lone: bool):
    """§5.6 Fourier on pairs of rows: f, g (m, n) float32 (g zeros for lone rows)."""
    m, n = f.shape
    out_f = np.empty_like(f)
    out_g = np.empty_like(g)
    big = ((af < -n) | (af > n)) & ((ag < -n) | (ag > n))
    row = np.zeros((m, L), np.complex64)
    row[:, :n] = f + 1j * g
    X = np.fft.fft(row.astype(np.complex128), axis=1).astype(np.complex64)
    Xr, Xi = X.real.astype(F32), X.imag.astype(F32)
    h = L // 2
    Fr = np.zeros((m, h + 1), F32)
    Fi = np.zeros((m, h + 1), F32)
    Gr = np.zeros((m, h + 1), F32)
    Gi = np.zeros((m, h + 1), F32)
    Fr[:, 0] = F32(2) * Xr[:, 0]
    Gr[:, 0] = F32(2) * Xi[:, 0]
    k = np.arange(1, h)
    Fr[:, 1:h] = Xr[:, k] + Xr[:, L - k]
    Fi[:, 1:h] = Xi[:, k] - Xi[:, L - k]
    Gr[:, 1:h] = Xi[:, k] + Xi[:, L - k]
    Gi[:, 1:h] = -Xr[:, k] + Xr[:, L - k]
    Fr[:, h] = F32(2) * Xr[:, h]
    Gr[:, h] = F32(2) * Xi[:, h]
    dk = F32((2.0 * PI) / L)
    for A, B, a in ((Fr, Fi, af), (Gr, Gi, ag)):
        phi = (-a * dk).astype(F32)
        er = np.array([F32(math.cos(F64(p))) for p in phi], F32)
        ei = np.array([F32(math.sin(F64(p))) for p in phi], F32)
        pr = np.ones(m, F32)
        pi = np.zeros(m, F32)
        for kk in range(1, h + 1):
            pr, pi = (er * pr - ei * pi).astype(F32), (er * pi + ei * pr).astype(F32)
            ar, ai = A[:, kk].copy(), B[:, kk].copy()
            A[:, kk] = (pr * ar - pi * ai).astype(F32)
            B[:, kk] = (pr * ai + pi * ar).astype(F32)
    Fi[:, h] = 0
    Gi[:, h] = 0
    Y = np.zeros((m, L), np.complex64)
    Y[:, 0] = Fr[:, 0] + 1j * Gr[:, 0]
    Y[:, k] = (Fr[:, k] - Gi[:, k]) + 1j * (Fi[:, k] + Gr[:, k])
    Y[:, L - k] = (Fr[:, k] + Gi[:, k]) + 1j * (-Fi[:, k] + Gr[:, k])
    Y[:, h] = Fr[:, h] + 1j * Gr[:, h]
    y = (np.fft.ifft(Y.astype(np.complex128), axis=1) * L).astype(np.complex64)
    sf = F32(0.5 / L)
    out_f[:] = (sf * y.real[:, :n].astype(F32)).astype(F32)
    out_g[:] = (sf * y.imag[:, :n].astype(F32)).astype(F32)
    out_f[big] = 0
    out_g[big] = 0
    return out_f, out_g


def apply_shear(v: np.ndarray, sh: Shear, method: str) -> np.ndarray:
    """§5.5: one shear on the padded volume v[k, j, i]."""
    nz, ny, nx = v.shape
    N = (nx, ny, nz)
    ax = sh.axis
    others = [a for a in range(3) if a != ax]  # (y, z) of §5.5 in index order
    a, b = F32(sh.coef[others[0]]), F32(sh.coef[others[1]])
    s = F32(sh.shift)
    c0 = F32(0.5 * (N[others[0]] - 1))
    c1 = F32(0.5 * (N[others[1]] - 1))
    st = F32(abs(F64(a)) * F64(c0) + abs(F64(b)) * F64(c1) + abs(F64(s)))
    if st < 1e-3:
        return v
    # move the shear axis last, others[0] middle, others[1] first: w[o1, o0, row]
    perm = [2 - others[1], 2 - others[0], 2 - ax]
    w = np.ascontiguousarray(v.transpose(perm))
    n1, n0, n = w.shape
    idx0 = np.arange(n0, dtype=F32)[None, :]
    idx1 = np.arange(n1, dtype=F32)[:, None]
    a0 = ((a * (idx0 - c0)).astype(F32) + (b * (idx1 - c1)).astype(F32)).astype(F32)
    a0 = (a0 + s).astype(F32)
    # §5.5: the second row of each pair is shifted by the first row's a0 plus a (float32)
    npair = n0 // 2
    a0[:, 1 : 2 * npair : 2] = (a0[:, 0 : 2 * npair : 2] + a).astype(F32)
    if method != "fourier":
        out = shift_rows_poly(w.reshape(-1, n), a0.reshape(-1), method).reshape(w.shape)
    else:
        L = _length(int(n + 0.5 * F64(st)))
        out = np.empty_like(w)
        # pairs along others[0]
        f = w[:, 0 : 2 * npair : 2, :].reshape(-1, n)
        g = w[:, 1 : 2 * npair : 2, :].reshape(-1, n)
        af = a0[:, 0 : 2 * npair : 2].reshape(-1)
        ag = a0[:, 1 : 2 * npair : 2].reshape(-1)
        if npair:
            of, og = fourier_pairs(f, g, af, ag, L, False)
            out[:, 0 : 2 * npair : 2, :] = of.reshape(n1, npair, n)
            out[:, 1 : 2 * npair : 2, :] = og.reshape(n1, npair, n)
        if n0 % 2 == 1:
            j = n0 - 1
            f1 = w[:, j, :]
            af1 = a0[:, j]
            if ax == 0:
                ag1 = (af1 + a).astype(F32)
            else:
                ag1 = a0[:, j - 1] if j >= 2 else np.zeros(n1, F32)
            of, _ = fourier_pairs(f1, np.zeros_like(f1), af1, ag1, L, True)
            out[:, j, :] = of
    inv = np.argsort(perm)
    return np.ascontiguousarray(out.transpose(inv))


def rotate(v: np.ndarray, delta, Q, d, method: str, zpad: int) -> np.ndarray:
    """§5: v[k, j, i] float32 moved by x -> Q x + d (mm, dataset axes)."""
    nz, ny, nx = v.shape
    if nx < 2 or ny < 2 or nz < 2:
        return v.copy()
    flip, sh = shears_for(Q, d, delta)
    if sh is None:
        return v.copy()
    w = np.pad(v, zpad) if zpad > 0 else v.copy()
    if flip is not None:
        for a in flip:
            w = np.flip(w, axis=2 - a)
        w = np.ascontiguousarray(w)
    for s in sh:
        w = apply_shear(w, s, method)
    if zpad > 0:
        w = w[zpad:-zpad, zpad:-zpad, zpad:-zpad]
    return np.ascontiguousarray(w).astype(F32)


def rotate_angles(v, delta, axes, th, d, method, zpad):
    """§5.1 from angles: th radians (float32), d mm (float32)."""
    th = [F32(t) for t in th]
    if th[0] == 0 and th[1] == 0 and th[2] == 0:
        th = [F32(1e-6), F32(1.1e-6), F32(0.9e-6)]
    Q = R3(axes[0], -F64(th[0]), axes[1], -F64(th[1]), axes[2], -F64(th[2]))
    return rotate(v, delta, Q, [F64(F32(x)) for x in d], method, zpad)


def rotate_matrix(v, delta, M, d, method, zpad):
    U, _, Vh = np.linalg.svd(M)
    return rotate(v, delta, U @ Vh, d, method, zpad)


# ------------------------------------------------------------------------------------------
# §6 the FIR Gaussian


def fir_weights(m: int, u: float) -> np.ndarray:
    u = F32(u)
    s = []
    N = F32(0)
    for i in range(m + 1):
        acc = 0.0
        for j in range(-11, 12):
            x = F64(F32(i * u)) + j * (F64(u) / 22.0)
            e = math.exp(-0.5 * x * x)
            acc += 0.5 * e if j in (-11, 11) else e
        s.append(acc)
        N = F32(F64(N) + acc) if i == 0 else F32(F64(N) + 2.0 * acc)
    r = F32(F32(1) / N)
    return np.array([F32(F32(x) * r) for x in s], F32)


def fir_blur_axis(v: np.ndarray, a: int, w: np.ndarray, wide512: bool = False) -> np.ndarray:
    """Blur v[k, j, i] along AFNI axis a with taps w (half-width m) in place semantics."""
    m = len(w) - 1
    ax = 2 - a
    n = v.shape[ax]
    if n < m + 1:
        return v
    x = np.moveaxis(v, ax, -1)
    ext = np.zeros((*x.shape[:-1], n + 2 * m), F32)
    ext[..., m : m + n] = x
    if a == 1 and wide512:
        ext[..., m - 1] = x[..., 1]
        ext[..., m + n] = x[..., n - 2]
    else:
        for q in range(1, m + 1):
            ext[..., m - q] = x[..., q]
            if a == 2:
                ext[..., m + n - 1 + q] = x[..., n - 1 - q]
            else:
                ext[..., m + n - 1 + q] = x[..., n - q]
    c = ext[..., m : m + n]
    if m <= 7:
        acc = None
        for q in range(m, 0, -1):
            t = (
                w[q] * (ext[..., m - q : m - q + n] + ext[..., m + q : m + q + n]).astype(F32)
            ).astype(F32)
            acc = t if acc is None else (acc + t).astype(F32)
        acc = (acc + (w[0] * c).astype(F32)).astype(F32)
    else:
        acc = (w[0] * c).astype(F32)
        for q in range(1, m + 1):
            t = (
                w[q] * (ext[..., m - q : m - q + n] + ext[..., m + q : m + q + n]).astype(F32)
            ).astype(F32)
            acc = (acc + t).astype(F32)
    return np.ascontiguousarray(np.moveaxis(acc, -1, ax)).astype(F32)


def blur(v: np.ndarray, sigma, delta, coarse: bool = False) -> np.ndarray:
    """§6.1 / §6.3 (FIR only); sigma, delta per axis (float32)."""
    out = v.astype(F32).copy()
    nz, ny, nx = v.shape
    N = (nx, ny, nz)
    for a in range(3):
        if N[a] < 2:
            continue
        m = math.ceil(2.5 * F64(F32(sigma[a])) / F64(F32(delta[a])))
        if m > 35 and not coarse:
            raise NotImplementedError("the FFT blur is not modelled")
        m = max(m, 1)
        if coarse:
            m = min(m, N[a] // 2)
        w = fir_weights(m, F32(F32(delta[a]) / F32(sigma[a])))
        out = fir_blur_axis(out, a, w, wide512=(nx >= 512))
    return out


# ------------------------------------------------------------------------------------------
# §7 the basis and the fit


def seqsum64(x: np.ndarray) -> float:
    x = np.asarray(x, F64).ravel()
    return float(np.cumsum(x)[-1]) if x.size else 0.0


def seqsum32(x: np.ndarray) -> F32:
    x = np.asarray(x, F32).ravel()
    return F32(np.cumsum(x, dtype=F32)[-1]) if x.size else F32(0)


def _largest_cluster(m: np.ndarray) -> np.ndarray:
    lab, nlab = ndimage.label(m, structure=ndimage.generate_binary_structure(3, 1))
    if nlab == 0:
        return np.zeros_like(m, bool)
    counts = np.bincount(lab.ravel())[1:]
    return lab == (int(np.argmax(counts)) + 1)


def _peel(m: np.ndarray) -> np.ndarray:
    """One peel and restore, 18 neighbours, threshold 17 (specs/3dAutomask.md §5.3)."""
    if m.size < 27:
        return m
    mm = m.astype(np.int32)
    pad = np.pad(mm, 1, mode="edge")
    nz, ny, nx = m.shape

    def count(arr_pad):
        c = np.zeros(m.shape, np.int32)
        for dz in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    if (dz, dy, dx) == (0, 0, 0) or abs(dz) + abs(dy) + abs(dx) == 3:
                        continue
                    c += arr_pad[1 + dz : 1 + dz + nz, 1 + dy : 1 + dy + ny, 1 + dx : 1 + dx + nx]
        return c

    marked = m & (count(pad) < 17)
    after = m & ~marked
    pad2 = np.pad(after.astype(np.int32), 1, mode="edge")
    restore = marked & (count(pad2) > 0)
    return after | restore


def trim_box(w: np.ndarray):
    nz, ny, nx = w.shape
    m = w != 0
    if not m.any():
        return None
    m = _largest_cluster(m)
    m = _peel(m)
    m = _largest_cluster(m)
    if not m.any():
        return None
    ks, js, is_ = np.nonzero(m)
    box = (is_.min(), is_.max(), js.min(), js.max(), ks.min(), ks.max())
    xa, xb, ya, yb, za, zb = box
    if not (xb > 0 or yb > 0 or zb > 0):
        return None
    nttt = F32((xb - xa + 1) * (yb - ya + 1) * (zb - za + 1))
    trat = F32(100.0 * F64(nttt) / F64(F32(nx * ny * nz)))
    return box if trat < 90.0 else None


def cut(v, box):
    if box is None:
        return v
    xa, xb, ya, yb, za, zb = box
    return v[za : zb + 1, ya : yb + 1, xa : xb + 1]


@dataclass
class Settings:
    method: str = "heptic"
    final: str | None = None
    clip: bool = True
    zpad: int = 4
    maxite: int = 23
    x_t: float = 0.01
    phi_t: float = 0.02
    dfac: float = 0.70
    twopass: bool = False
    twoblur: float = 2.0
    sinit_on: bool = True
    coarse_del: int = 10
    coarse_num: int = 2
    coarse_rot: bool = True
    edging: tuple = ("percent", 5.0)
    maxdisp: bool = True
    base_index: int = 0


@dataclass
class State:
    sinit: F32 = field(default_factory=lambda: F32(1))
    clfrac: float = 0.333
    log: list = field(default_factory=list)


class Basis:
    def __init__(self, b, delta, axes, weight, edges, wproc, method, dfac, zpad, st: State):
        self.delta, self.axes, self.method, self.zpad = delta, axes, method, zpad
        nz, ny, nx = b.shape
        if weight is None or wproc:
            src = b if weight is None else weight
            w = np.abs(src).astype(F32)
            sig = [F32(3.0 * F64(delta[a])) for a in range(3)]
            w = blur(w, sig, delta)
            clip = F32(0.025 * F64(w.max()))
            w = np.where(w < clip, F32(0), w).astype(F32)
        else:
            w = weight.astype(F32).copy()
        if edges is not None:
            ex, ey, ez = edges
            if ez > 0:
                w[:ez] = 0
                w[nz - ez :] = 0
            if ex > 0:
                w[:, :, :ex] = 0
                w[:, :, nx - ex :] = 0
            if ey > 0:
                w[:, :ey, :] = 0
                w[:, ny - ey :, :] = 0
        self.box = trim_box(w)
        eps_r = F32(2.0 * F64(F32(dfac)) / (nx + ny + nz))
        refs = [cut(b, self.box)]
        for p in range(3):
            th = [F32(0)] * 3
            th[p] = eps_r
            pp = rotate_angles(b, delta, axes, th, (0, 0, 0), method, zpad)
            th[p] = F32(-eps_r)
            qq = rotate_angles(b, delta, axes, th, (0, 0, 0), method, zpad)
            kap = F32(F64(st.sinit) * 0.5 * (PI / 180.0) / F64(eps_r))
            refs.append(cut((kap * (qq - pp).astype(F32)).astype(F32), self.box))
        for n in range(3):
            eps = F32(F32(dfac) * delta[n])
            d = [F32(0)] * 3
            d[n] = eps
            pp = rotate_angles(b, delta, axes, (0, 0, 0), d, method, zpad)
            d[n] = F32(-eps)
            qq = rotate_angles(b, delta, axes, (0, 0, 0), d, method, zpad)
            kap = F32(F64(st.sinit) * 0.5 / F64(eps))
            refs.append(cut((kap * (qq - pp).astype(F32)).astype(F32), self.box))
        wt = cut(w, self.box).ravel()
        R = [r.ravel().astype(F32) for r in refs]
        self.ok = all((r != 0).any() for r in R)
        C = np.zeros((7, 7))
        for j in range(7):
            for i in range(j + 1):
                C[i, j] = C[j, i] = seqsum64(((R[i] * R[j]).astype(F32) * wt).astype(F32))
        L = np.zeros((7, 7))
        for i in range(7):
            for j in range(i):
                s = C[i, j]
                for k in range(j):
                    s -= L[i, k] * L[j, k]
                L[i, j] = s / L[j, j]
            s = C[i, i]
            for k in range(i):
                s -= L[i, k] * L[i, k]
            if s <= 0:
                self.ok = False
                break
            L[i, i] = math.sqrt(s)
        self.L = L
        self.R = [(r * wt).astype(F32) for r in R]

    def lsq(self, y: np.ndarray) -> np.ndarray:
        y = cut(y, self.box).ravel().astype(F32)
        rr = [seqsum64((r * y).astype(F32)) for r in self.R]
        L = self.L
        for i in range(7):
            s = rr[i]
            for j in range(i):
                s -= L[i, j] * rr[j]
            rr[i] = s / L[i, i]
        for i in range(6, -1, -1):
            s = rr[i]
            for j in range(i + 1, 7):
                s -= L[j, i] * rr[j]
            rr[i] = s / L[i, i]
        return np.array(rr, F32)

    def register(self, y, init, x_t, phi_t, maxite, final=None, clip=False):
        dt = [F32(self.delta[n] * F32(x_t)) for n in range(3)]
        if init is not None and any(F32(v) != 0 for v in init):
            fit = np.array([1.0, *init], F32)
            good = True
        else:
            fit = self.lsq(y)
            good = any(10.0 * abs(F64(fit[1 + p])) > phi_t for p in range(3)) or any(
                10.0 * abs(F64(fit[4 + n])) > dt[n] for n in range(3)
            )
        it = 0
        while good:
            th = [F32(F64(fit[1 + p]) * (PI / 180.0)) for p in range(3)]
            t = rotate_angles(y, self.delta, self.axes, th, fit[4:7], self.method, self.zpad)
            df = self.lsq(t)
            fit[1:] = (fit[1:] + df[1:]).astype(F32)
            it += 1
            good = it < maxite and (
                any(abs(F64(df[1 + p])) > phi_t for p in range(3))
                or any(abs(F64(df[4 + n])) > dt[n] for n in range(3))
            )
        th = [F32(F64(fit[1 + p]) * (PI / 180.0)) for p in range(3)]
        out = None
        if final is not None:
            out = rotate_angles(y, self.delta, self.axes, th, fit[4:7], final, self.zpad)
            if clip and final in ("cubic", "quintic", "heptic", "fourier"):
                out = np.clip(out, y.min(), y.max()).astype(F32)
        return th, [F32(x) for x in fit[4:7]], out, it


# ------------------------------------------------------------------------------------------
# §8.6 the coarse searches


def shift_search(B, V, W, dl, num):
    nz, ny, nx = B.shape
    shtop = dl * num
    e = shtop + dl
    bsum = seqsum32((B * B).astype(F32))
    best = (0, 0, 0)
    for dz in range(-shtop, shtop + 1, dl):
        for dy in range(-shtop, shtop + 1, dl):
            for dx in range(-shtop, shtop + 1, dl):
                if dx * dx + dy * dy + dz * dz > shtop * shtop:
                    continue
                if nx - e <= e or ny - e <= e or nz - e <= e:
                    s = F32(0)
                else:
                    b = B[e : nz - e, e : ny - e, e : nx - e]
                    v = V[e - dz : nz - e - dz, e - dy : ny - e - dy, e - dx : nx - e - dx]
                    if W is not None:
                        w = W[e : nz - e, e : ny - e, e : nx - e]
                        sel = w > 0
                        b, v, w = b[sel], v[sel], w[sel]
                        sbb = seqsum32(((w * b).astype(F32) * b).astype(F32))
                        svv = seqsum32(((w * v).astype(F32) * v).astype(F32))
                        sbv = seqsum32(((w * b).astype(F32) * v).astype(F32))
                    else:
                        sbb = seqsum32((b * b).astype(F32))
                        svv = seqsum32((v * v).astype(F32))
                        sbv = seqsum32((b * v).astype(F32))
                    s = sbb
                    if svv > 0:
                        s = F32(sbb - F32(F32(sbv * sbv) / svv))
                if s < bsum:
                    bsum, best = s, (dx, dy, dz)
    return bsum, best


def automask(vol_kji: np.ndarray, clfrac: float) -> np.ndarray:
    import larmorx as lx

    arr = np.ascontiguousarray(vol_kji.transpose(2, 1, 0))
    r = lx.afni.automask((arr, np.eye(4)), clfrac=clfrac, implementation="original")
    m = np.asarray(r.mask.data if hasattr(r.mask, "data") else r.mask)
    return np.ascontiguousarray(m.transpose(2, 1, 0)) != 0


def shiftrot_search(grid: Grid, B, V, dl, num, delta, zpad, st: State):
    nz, ny, nx = B.shape
    sub = nx >= 120 and ny >= 120 and nz >= 120
    one = (F32(1), F32(1), F32(1))
    if sub:

        def half(x):
            t = blur(x, one, one, coarse=True)
            return np.ascontiguousarray(t[::2, ::2, ::2])

        B, V = half(B), half(V)
        dl = dl // 2
        delta = [F32(2 * d) for d in delta]
    W = blur(np.abs(B).astype(F32), one, one, coarse=True)
    top = F32(F32(1) / W.max())
    W = (W * top).astype(F32)
    W = np.where(W < 0.05, F32(0), W).astype(F32)
    mk = automask(W, st.clfrac)
    W = np.where(mk, W, F32(0)).astype(F32)
    h = grid.handedness()
    sig = [np.sign(grid.code(L)) * h for L in "IRA"]
    axes = [abs(grid.code(L)) - 1 for L in "IRA"]
    best = ((0.0, 0.0, 0.0), (0, 0, 0))
    bsum = F32(1e38)
    for a in (-1, 0, 1):
        for b in (-2, -1, 0, 1, 2):
            for c in (-1, 0, 1):
                r, p, y = 9.0 * a, 9.0 * b, 9.0 * c
                if r == 0 and p == 0 and y == 0:
                    T = V
                else:
                    M = R3(
                        axes[0],
                        -(PI / 180.0) * (sig[0] * r),
                        axes[1],
                        -(PI / 180.0) * (sig[1] * p),
                        axes[2],
                        -(PI / 180.0) * (sig[2] * y),
                    )
                    T = rotate_matrix(V, delta, M, [0.0, 0.0, 0.0], "nn", zpad)
                s, sh = shift_search(B, T, W, dl, num)
                if s < bsum:
                    bsum, best = s, ((r, p, y), sh)
    (r, p, y), sh = best
    if sub:
        sh = tuple(2 * x for x in sh)
    return (r, p, y), sh


# ------------------------------------------------------------------------------------------
# §8 the program


@dataclass
class Result:
    params: list  # per volume (roll, pitch, yaw, dS, dL, dP) float32
    registered: list
    kind: str
    matrices: list
    maxdisp: list
    maxdelta: list
    rms: list


def volreg(path: str, s: Settings, st: State | None = None) -> Result:
    st = st or State()
    img = nib.load(path)
    grid = read_grid(img)
    vols, kind = read_volumes(img)
    nx, ny, nz = grid.n
    delta = grid.delta
    h = grid.handedness()
    axes = [abs(grid.code(L)) - 1 for L in "IRA"]
    sig = [int(np.sign(grid.code(L))) * h for L in "IRA"]
    base = vols[s.base_index]
    if s.edging[0] == "percent":
        edges = tuple(int(0.01 * F64(F32(s.edging[1])) * N + 0.5) for N in (nx, ny, nz))
    else:
        edges = tuple(int(min(0.25 * N, F64(F32(s.edging[1])))) for N in (nx, ny, nz))
    final = s.final or s.method
    dl, num = s.coarse_del, s.coarse_num
    if s.twopass and dl > 0 and num > 0:
        mm = max(int(0.1 * min(nx, ny, nz) + 0.499), 1)
        dl = min(dl, mm)
    if not s.maxdisp:
        st.clfrac = 0.5
    maxite = s.maxite
    if s.twopass and maxite < 10:
        maxite = 66
    init = [None] * len(vols)
    one = [F32(1)] * 3
    tb = [F32(s.twoblur)] * 3
    if s.twopass:
        B1 = blur(base, tb, one)
        V0 = blur(vols[0], tb, one)
        rot0, sh0 = ((0.0, 0.0, 0.0), (0, 0, 0))
        if dl > 0 and num > 0:
            if s.coarse_rot:
                rot0, sh0 = shiftrot_search(grid, B1, V0, dl, num, delta, s.zpad, st)
            else:
                _, sh0 = shift_search(B1, V0, None, dl, num)
        sx, sy, sz = sh0
        bx, by, bz = abs(sx), abs(sy), abs(sz)
        boxs = (slice(bz, nz - bz), slice(by, ny - by), slice(bx, nx - bx))
        shifted = np.zeros_like(V0)
        shifted[boxs] = V0[bz - sz : nz - bz - sz, by - sy : ny - by - sy, bx - sx : nx - bx - sx]
        if s.sinit_on:
            fsum = seqsum32(shifted[boxs])
            gsum = seqsum32(B1[boxs])
            if gsum > 0:
                sc = F32(fsum / gsum)
                if 0.005 < sc < 2000.0:
                    st.sinit = sc
        W1 = B1.copy()
        W1[boxs] = (W1[boxs] + shifted[boxs]).astype(F32)
        W1 = blur(W1, tb, one)
        clip = F32(0.025 * F64(W1.max()))
        W1 = np.where(clip > W1, F32(0), W1).astype(F32)
        bas1 = Basis(
            B1, delta, axes, W1, edges, False, "linear", F32(s.twoblur * F32(s.dfac)), s.zpad, st
        )
        for k, v in enumerate(vols):
            if k == s.base_index or np.count_nonzero(v) < 66:
                init[k] = [0.0] * 6
                continue
            Vk = blur(v, tb, one)
            if k == 0:
                rot, sh = rot0, sh0
            elif dl > 0 and num > 0:
                if s.coarse_rot:
                    rot, sh = shiftrot_search(grid, B1, Vk, dl, num, delta, s.zpad, st)
                else:
                    _, sh = shift_search(B1, Vk, None, dl, num)
                    rot = (0.0, 0.0, 0.0)
            else:
                rot, sh = (0.0, 0.0, 0.0), (0, 0, 0)
            iv = [F32(rot[p] * sig[p]) for p in range(3)] + [
                F32(sh[n] * delta[n]) for n in range(3)
            ]
            th, dd, _, _ = bas1.register(
                Vk, iv, F32(s.twoblur * F32(s.x_t)), F32(s.twoblur * F32(s.phi_t)), maxite
            )
            init[k] = [F32(F64(t) * (180.0 / PI)) for t in th] + dd
    bas = Basis(base, delta, axes, None, edges, False, s.method, F32(s.dfac), s.zpad, st)
    res = Result([], [], kind, [], [], [], [])
    P = grid.P()
    ci = grid.centre()
    cb = ci
    for k, v in enumerate(vols):
        if k == s.base_index:
            th, dd, out = [F32(0)] * 3, [F32(0)] * 3, base.copy()
        elif np.count_nonzero(v) < 66:
            th, dd, out = [F32(0)] * 3, [F32(0)] * 3, v.copy()
        else:
            th, dd, out, _ = bas.register(
                v, init[k] if s.twopass else None, F32(s.x_t), F32(s.phi_t), maxite, final, s.clip
            )
        ang = [F32(F64(t) * (180.0 / PI)) for t in th]
        rep_ang = [F32(ang[p]) if sig[p] > 0 else F32(-ang[p]) for p in range(3)]
        dS = dL = dP = F32(0)
        for n, o in enumerate(grid.orient):
            x = dd[n]
            if o == "R2L":
                dL = F32(x)
            elif o == "L2R":
                dL = F32(-x)
            elif o == "P2A":
                dP = F32(-x)
            elif o == "A2P":
                dP = F32(x)
            elif o == "I2S":
                dS = F32(x)
            else:
                dS = F32(-x)
        roll, pitch, yaw = rep_ang
        res.params.append((roll, pitch, yaw, dS, dL, dP))
        res.registered.append(store(out, kind))
        # rms (§8.7 item 6)
        n = out.size
        rn = F32(math.sqrt(F64(F32(seqsum32(((base - out) ** 2).astype(F32)) / F32(n)))))
        ro = F32(math.sqrt(F64(F32(seqsum32(((base - v) ** 2).astype(F32)) / F32(n)))))
        res.rms.append((ro, rn))
        # §4.5 the matrices
        Rm = R3(
            2, -(PI / 180.0) * F64(roll), 0, -(PI / 180.0) * F64(pitch), 1, -(PI / 180.0) * F64(yaw)
        )
        t = np.array([dL, dP, dS], F32)
        w = np.array(
            [
                F32(Rm[i, 0] * F64(ci[0]) + Rm[i, 1] * F64(ci[1]) + Rm[i, 2] * F64(ci[2]))
                for i in range(3)
            ],
            F32,
        )
        vv = np.array([F32(t[i] + F32(cb[i] - w[i])) for i in range(3)], F64)
        A = Rm
        det = (
            A[0, 0] * A[1, 1] * A[2, 2]
            - A[0, 0] * A[1, 2] * A[2, 1]
            - A[1, 0] * A[0, 1] * A[2, 2]
            + A[1, 0] * A[0, 2] * A[2, 1]
            + A[2, 0] * A[0, 1] * A[1, 2]
            - A[2, 0] * A[0, 2] * A[1, 1]
        )
        r = 1.0 / det
        inv = np.array(
            [
                [
                    (A[1, 1] * A[2, 2] - A[1, 2] * A[2, 1]) * r,
                    (-A[0, 1] * A[2, 2] + A[0, 2] * A[2, 1]) * r,
                    (A[0, 1] * A[1, 2] - A[0, 2] * A[1, 1]) * r,
                ],
                [
                    (-A[1, 0] * A[2, 2] + A[1, 2] * A[2, 0]) * r,
                    (A[0, 0] * A[2, 2] - A[0, 2] * A[2, 0]) * r,
                    (-A[0, 0] * A[1, 2] + A[0, 2] * A[1, 0]) * r,
                ],
                [
                    (A[1, 0] * A[2, 1] - A[1, 1] * A[2, 0]) * r,
                    (-A[0, 0] * A[2, 1] + A[0, 1] * A[2, 0]) * r,
                    (A[0, 0] * A[1, 1] - A[0, 1] * A[1, 0]) * r,
                ],
            ]
        )
        u = matvec(inv, vv)
        u = -u
        res.matrices.append(
            [
                inv[0, 0],
                inv[0, 1],
                inv[0, 2],
                u[0],
                inv[1, 0],
                inv[1, 1],
                inv[1, 2],
                u[1],
                inv[2, 0],
                inv[2, 1],
                inv[2, 2],
                u[2],
            ]
        )
    # §8.4 / §8.8 maximum displacements
    if s.maxdisp:
        import larmorx as lx

        r = lx.afni.automask(path, clfrac=0.333, implementation="original")
        m = np.asarray(r.mask.data if hasattr(r.mask, "data") else r.mask)
        m = np.ascontiguousarray(m.transpose(2, 1, 0)) != 0
        pad = np.pad(m, 1, constant_values=False)
        inner = (
            pad[2:, 1:-1, 1:-1]
            & pad[:-2, 1:-1, 1:-1]
            & pad[1:-1, 2:, 1:-1]
            & pad[1:-1, :-2, 1:-1]
            & pad[1:-1, 1:-1, 2:]
            & pad[1:-1, 1:-1, :-2]
        )
        face = np.zeros_like(m)
        face[0], face[-1], face[:, 0], face[:, -1], face[:, :, 0], face[:, :, -1] = (
            True,
            True,
            True,
            True,
            True,
            True,
        )
        edge = m & (~inner | face)
        ks, js, is_ = np.nonzero(edge)
        X = np.stack(
            [
                (grid.org[0] + (is_.astype(F32) * grid.step[0]).astype(F32)).astype(F32),
                (grid.org[1] + (js.astype(F32) * grid.step[1]).astype(F32)).astype(F32),
                (grid.org[2] + (ks.astype(F32) * grid.step[2]).astype(F32)).astype(F32),
            ],
            axis=1,
        ).astype(F64)
        c = grid.axis_centre().astype(F64)
        prev = None
        for roll, pitch, yaw, dS, dL, dP in res.params:
            Rm = R3(
                2,
                -(PI / 180.0) * F64(roll),
                0,
                -(PI / 180.0) * F64(pitch),
                1,
                -(PI / 180.0) * F64(yaw),
            )
            Rm32 = Rm  # double
            t = np.array([dL, dP, dS], F32).astype(F64)
            Rd = P.T @ Rm32 @ P
            td = P.T @ t
            q = (X - c) @ Rd.T + td + c
            u = X - q
            dmax = float(np.max(np.sqrt(np.sum(u * u, axis=1)).astype(F32))) if len(u) else 0.0
            if prev is None:
                emax = 0.0
            else:
                e = prev - u
                emax = float(np.max(np.sqrt(np.sum(e * e, axis=1)).astype(F32)))
            prev = u
            res.maxdisp.append(dmax)
            res.maxdelta.append(emax)
    return res


def write_1d(res: Result, path: str) -> None:
    with open(path, "w", encoding="utf-8") as f:
        for p in res.params:
            f.write("%8.4f %8.4f %8.4f %8.4f %8.4f %8.4f\n" % tuple(float(x) for x in p))  # noqa: UP031 (C's printf)


def write_matrices(res: Result, path: str) -> None:
    with open(path, "w", encoding="utf-8") as f:
        f.write("# 3dvolreg matrices (DICOM-to-DICOM, row-by-row):\n")
        for m in res.matrices:
            f.write(" ".join("%13.6g" % x for x in m) + "\n")  # noqa: UP031 (C's printf)
