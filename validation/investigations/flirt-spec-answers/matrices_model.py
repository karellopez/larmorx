# SPDX-License-Identifier: Apache-2.0
"""Models of FLIRT's matrix arithmetic, written from ``specs/flirt.md``'s text alone (the spec
writer's answers of 2026-11-02 to the clean-room questions 9, 11, 15 and 16: §3's composition
and decomposition, §4.1's left-right swap, §9.3's written header, §12's inverse), compared with
what FSL 6.0.7.23 wrote in ``fsl_runs.py``'s runs:

- ``inverse-noz`` (question 11): ``convert_xfm -inverse`` of an exact rotation about z with no z
  translation; FSL's text, its ``-0`` included;
- ``fixscaleskew`` (question 16): ``convert_xfm -fixscaleskew``; FSL's ten significant digits;
- ``usesqform`` (question 15): ``-applyxfm -usesqform``'s matrix (the reference is a
  neurological file);
- ``isoxfm-oblique`` (question 9): the qform and sform fields of ``-applyisoxfm 3.5``'s output
  on a neurological reference with an oblique qform and no sform.

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 4 \\
        python -P validation/investigations/flirt-spec-answers/matrices_model.py <runs> \\
        [--variant double-fov|plain-rotation|divide]

``--variant`` shows where another reading parts from FSL: ``double-fov`` (the field of view of
the swap in double precision), ``plain-rotation`` (each elementary rotation as the textbook
matrix of single-precision sines and cosines), ``divide`` (the decomposition's scale removal as
a division instead of the reciprocal's product).
"""

from __future__ import annotations

import ctypes
import ctypes.util
import sys
from pathlib import Path

import numpy as np

F32 = np.float32
LD = np.longdouble
_libm = ctypes.CDLL(ctypes.util.find_library("m"))
for _name in ("sinf", "cosf"):
    getattr(_libm, _name).restype = ctypes.c_float
    getattr(_libm, _name).argtypes = [ctypes.c_float]
_libm.atan2f.restype = ctypes.c_float
_libm.atan2f.argtypes = [ctypes.c_float, ctypes.c_float]

VARIANT = ""


def f32(x: float) -> float:
    """``x`` rounded to single precision, as a Python float (exact)."""
    return float(F32(x))


def sinf(x: float) -> float:
    return float(_libm.sinf(x))


def cosf(x: float) -> float:
    return float(_libm.cosf(x))


def atan2f(y: float, x: float) -> float:
    return float(_libm.atan2f(y, x))


# --- §12: the small products and sums ------------------------------------------------------


def accu(v: list[float]) -> float:
    """A sum with two interleaved accumulators (even and odd positions), then added."""
    a, b = 0.0, 0.0
    for i, x in enumerate(v):
        if i % 2 == 0:
            a += x
        else:
            b += x
    return a + b


def dot(a: list[float], b: list[float]) -> float:
    return accu([x * y for x, y in zip(a, b, strict=True)])


def sumsq(a: list[float]) -> float:
    return accu([x * x for x in a])


def mul(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """A small square product: each element the sum over the inner index, in order, of the
    rounded products (the first product, then each next one added)."""
    n = a.shape[0]
    out = np.zeros((n, n))
    for r in range(n):
        for c in range(n):
            s = a[r, 0] * b[0, c]
            for k in range(1, n):
                s = s + a[r, k] * b[k, c]
            out[r, c] = s
    return out


def matvec(a: np.ndarray, x: list[float]) -> list[float]:
    n = a.shape[0]
    out = []
    for r in range(n):
        s = a[r, 0] * x[0]
        for k in range(1, len(x)):
            s = s + a[r, k] * x[k]
        out.append(s)
    return out


def mul4(a, b, c, d):
    """Four factors: ((A B) C) D."""
    return mul(mul(mul(a, b), c), d)


# --- §12: the inverse of a 4 x 4 matrix ----------------------------------------------------


def is_diag(a):
    return all(a[r, c] == 0.0 for r in range(a.shape[0]) for c in range(a.shape[0]) if r != c)


def is_triu(a):
    return all(a[r, c] == 0.0 for r in range(a.shape[0]) for c in range(r))


def scal(d: float, x: list[float]) -> list[float]:
    return [0.0 if d == 0.0 else d * v for v in x]


def trtri_upper(a: np.ndarray) -> np.ndarray:
    """The inverse of an upper triangle, column by column: the diagonal's reciprocal, the
    column above it multiplied by the part already inverted (for each earlier column i in
    order: the entries above i get x_i times that column's entries added after their product
    is rounded, then x_i times the diagonal), then scaled by minus the reciprocal."""
    a = a.copy()
    n = a.shape[0]
    for j in range(n):
        ajj = 1.0 / a[j, j]
        a[j, j] = ajj
        x = [a[k, j] for k in range(j)]
        for i in range(j):
            if i > 0:
                xi = x[i]
                for k in range(i):
                    x[k] = xi * a[k, i] + x[k]
            x[i] = x[i] * a[i, i]
        x = scal(-ajj, x)
        for k in range(j):
            a[k, j] = x[k]
    return a


def gemv_minus(y: list[float], cols: list[list[float]], x: list[float], rows: int) -> list[float]:
    """y - A x for the LU's and the inverse's updates (alpha = -1): with four rows, the columns
    in pairs (the second product plus the first, subtracted from y in one rounding) and a last
    single column as y + a·(x·(-1)); with fewer rows, the dot from 0 in order, then
    y + (-1)·t."""
    y = list(y)
    if rows == 4:
        k = 0
        if len(cols) & 2:
            for r in range(4):
                s = x[1] * cols[1][r] + x[0] * cols[0][r]
                y[r] = y[r] - s
            k = 2
        if len(cols) & 1:
            xa = x[k] * -1.0
            for r in range(4):
                y[r] = y[r] + cols[k][r] * xa
        return y
    for r in range(rows):
        t = 0.0
        for k, xk in enumerate(x):
            t = t + xk * cols[k][r]
        y[r] = t * -1.0 + y[r]
    return y


def inv(a: np.ndarray) -> np.ndarray:
    """§12's inverse: diagonal → reciprocals; triangular → the triangle's inverse; otherwise the
    LU factorisation with partial pivoting (left-looking) and the inverse from it."""
    n = a.shape[0]
    if is_diag(a):
        out = np.zeros_like(a)
        for i in range(n):
            out[i, i] = 1.0 / a[i, i]
        return out
    if is_triu(a):
        return trtri_upper(a)
    a = a.copy()
    piv = [0] * n
    for j in range(n):
        for i in range(j):
            ip = piv[i]
            if ip != i:
                a[i, j], a[ip, j] = a[ip, j], a[i, j]
        for i in range(1, j):
            t = 0.0
            for k in range(i):
                t = t + a[k, j] * a[i, k]
            t = t + 0.0
            a[i, j] = a[i, j] - t
        y = gemv_minus(
            [a[r, j] for r in range(j, n)],
            [[a[r, k] for r in range(j, n)] for k in range(j)],
            [a[k, j] for k in range(j)],
            n - j,
        )
        for r in range(j, n):
            a[r, j] = y[r - j]
        jp = j + int(np.argmax([abs(a[r, j]) for r in range(j, n)]))
        piv[j] = jp
        p = a[jp, j]
        if p != 0.0:
            rcp = 1.0 / p
            if jp != j:
                for c in range(j + 1):
                    a[j, c], a[jp, c] = a[jp, c], a[j, c]
            for r in range(j + 1, n):
                a[r, j] = rcp * a[r, j]
    u = np.triu(a)
    ui = trtri_upper(u)
    for r in range(n):
        for c in range(r, n):
            a[r, c] = ui[r, c]
    for j in range(n - 1, -1, -1):
        work = [a[i, j] for i in range(j + 1, n)]
        for i in range(j + 1, n):
            a[i, j] = 0.0
        if j + 1 < n:
            y = gemv_minus(
                [a[r, j] for r in range(n)],
                [[a[r, k] for r in range(n)] for k in range(j + 1, n)],
                work,
                n,
            )
            for r in range(n):
                a[r, j] = y[r]
    for j in range(n - 2, -1, -1):
        jp = piv[j]
        if jp != j:
            a[:, [j, jp]] = a[:, [jp, j]]
    return a


# --- §3: the composition and the decomposition ---------------------------------------------


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def elementary_rotation(axis: int, angle: float, centre: list[float]) -> np.ndarray:
    rot = np.eye(4)
    a = [0.0, 0.0, 0.0]
    a[axis] = angle
    theta = f32(np.sqrt(sumsq(a)))
    if theta < 1e-8:
        return rot
    if VARIANT == "plain-rotation":
        c, s = cosf(f32(angle)), sinf(f32(angle))
        r3 = np.eye(3)
        i, j = [(1, 2), (0, 2), (0, 1)][axis]
        sgn = -1.0 if axis == 1 else 1.0
        r3[i, i], r3[j, j], r3[i, j], r3[j, i] = c, c, sgn * s, -sgn * s
    else:
        u = [v / theta for v in a]
        v = [-u[1], u[0], 0.0]
        if np.sqrt(sumsq(v)) <= 0.0:
            v = [1.0, 0.0, 0.0]
        nv = np.sqrt(sumsq(v))
        v = [x / nv for x in v]
        w = cross(u, v)
        nw = np.sqrt(sumsq(w))
        w = [x / nw for x in w]
        b = np.array([[v[r], w[r], u[r]] for r in range(3)])
        c, s = cosf(theta), sinf(theta)
        cm = np.array([[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]])
        r3 = mul(mul(b, cm), b.T.copy())
    rot[:3, :3] = r3
    im = np.eye(3) - r3
    rot[:3, 3] = matvec(im, centre)
    return rot


def compose(p: list[float], centre: list[float]) -> np.ndarray:
    """12 parameters (rotations, translations, scales, skews) about ``centre``."""
    m = np.eye(4)
    for k in range(3):
        m = mul(m, elementary_rotation(k, p[k], centre))
    for k in range(3):
        m[k, 3] = m[k, 3] + p[3 + k]
    sc = np.eye(4)
    sc[0, 0], sc[1, 1], sc[2, 2] = p[6], p[7], p[8]
    t = matvec(sc[:3, :3], centre)
    for r in range(3):
        sc[r, 3] = centre[r] - t[r]
    sk = np.eye(4)
    sk[0, 1], sk[0, 2], sk[1, 2] = p[9], p[10], p[11]
    t = matvec(sk[:3, :3], centre)
    for r in range(3):
        sk[r, 3] = centre[r] - t[r]
    return mul(mul(m, sk), sc)


def decompose(m: np.ndarray, centre: list[float]) -> list[float]:
    x, y, z = ([m[r, c] for r in range(3)] for c in range(3))
    sx = F32(np.sqrt(sumsq(x)))
    dxy = F32(dot(x, y))
    sy = F32(np.sqrt(F32(F32(dot(y, y)) - F32(F32(dxy * dxy) / F32(sx * sx)))))
    a = F32(dxy / F32(sx * sy))
    x0 = [v / float(sx) for v in x]
    y0 = [v / float(sy) - float(a) * w for v, w in zip(y, x0, strict=True)]
    d0, d1 = F32(dot(x0, z)), F32(dot(y0, z))
    sz = F32(np.sqrt(F32(F32(F32(dot(z, z)) - F32(d0 * d0)) - F32(d1 * d1))))
    b, c = F32(d0 / sz), F32(d1 / sz)
    sx, sy, sz, a, b, c = (float(v) for v in (sx, sy, sz, a, b, c))
    # the skew's inverse (a unit upper triangle), then the scales removed row by row: the
    # reciprocal of each scale times the row (a solve of diag(s) against it)
    ki = trtri_upper(np.array([[1.0, a, b], [0.0, 1.0, c], [0.0, 0.0, 1.0]]))
    solved = np.zeros((3, 3))
    for r, s in enumerate((sx, sy, sz)):
        for col in range(3):
            solved[r, col] = ki[r, col] / s if VARIANT == "divide" else (1.0 / s) * ki[r, col]
    rm = mul(m[:3, :3].copy(), solved)
    t = matvec(m[:3, :3], centre)
    trans = [(t[i] + m[i, 3]) - centre[i] for i in range(3)]
    cy = f32(np.sqrt(rm[0, 0] * rm[0, 0] + rm[0, 1] * rm[0, 1]))
    if cy < 1e-4:
        ang = [atan2f(f32(-rm[2, 1]), f32(rm[1, 1])), atan2f(f32(-rm[0, 2]), 0.0), 0.0]
    else:
        cz, sz_ = f32(rm[0, 0] / cy), f32(rm[0, 1] / cy)
        cx, sx_ = f32(rm[2, 2] / cy), f32(rm[1, 2] / cy)
        sy_ = f32(-rm[0, 2])
        ang = [atan2f(sx_, cx), atan2f(sy_, cy), atan2f(sz_, cz)]
    return [*ang, *trans, sx, sy, sz, a, b, c]


# --- printing and reading ------------------------------------------------------------------


def fmt_matrix(m: np.ndarray) -> str:
    return "".join("".join(f"{v:.10g}  " for v in m[r]) + "\n" for r in range(4))


def read_matrix(p: Path) -> np.ndarray:
    return np.array(
        [[float(v) for v in ln.split()] for ln in p.read_text().split("\n") if ln.strip()]
    )


# --- §4.1: the left-right swap of a neurological file, §9.3: the written header ------------


def swap_lr(form: np.ndarray, nx: int, dims: tuple[float, float, float]) -> np.ndarray:
    """``W · D⁻¹ · F⁻¹ · D`` with ``F`` the flip whose translation is the field of view
    ``(n_x − 1)·d_x`` in single precision (``double-fov``: in double)."""
    d = np.diag([dims[0], dims[1], dims[2], 1.0])
    fov = float(nx - 1) * dims[0] if VARIANT == "double-fov" else f32(F32(nx - 1) * F32(dims[0]))
    flip = np.eye(4)
    flip[0, 0], flip[0, 3] = -1.0, fov
    return mul4(form, inv(d), inv(flip), d)


def quatern_to_mat44(qb, qc, qd, qx, qy, qz, dx, dy, dz, qfac) -> np.ndarray:
    """The NIfTI-1 reference library's conversion, its long double constants as x87 operations,
    every element stored in single precision."""
    b, c, d = float(F32(qb)), float(F32(qc)), float(F32(qd))
    a = float(LD(1.0) - LD(b * b + c * c + d * d))
    if a < 1e-7:
        s = float(LD(1.0) / LD(np.sqrt(b * b + c * c + d * d)))
        b, c, d, a = b * s, c * s, d * s, 0.0
    else:
        a = float(np.sqrt(a))
    xd = float(dx) if dx > 0 else 1.0
    yd = float(dy) if dy > 0 else 1.0
    zd = float(dz) if dz > 0 else 1.0
    if qfac < 0:
        zd = -zd
    r = np.eye(4)
    r[0, 0] = f32((a * a + b * b - c * c - d * d) * xd)
    r[0, 1] = f32(LD(2.0) * LD(b * c - a * d) * LD(yd))
    r[0, 2] = f32(LD(2.0) * LD(b * d + a * c) * LD(zd))
    r[1, 0] = f32(LD(2.0) * LD(b * c + a * d) * LD(xd))
    r[1, 1] = f32((a * a + c * c - b * b - d * d) * yd)
    r[1, 2] = f32(LD(2.0) * LD(c * d - a * b) * LD(zd))
    r[2, 0] = f32(LD(2.0) * LD(b * d - a * c) * LD(xd))
    r[2, 1] = f32(LD(2.0) * LD(c * d + a * b) * LD(yd))
    r[2, 2] = f32((a * a + d * d - c * c - b * b) * zd)
    r[0, 3], r[1, 3], r[2, 3] = f32(qx), f32(qy), f32(qz)
    return r


def mat33_inverse(r: np.ndarray) -> np.ndarray:
    (r11, r12, r13), (r21, r22, r23), (r31, r32, r33) = (map(float, row) for row in r)
    deti = (
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
    ) - r31 * r22 * r13
    if deti != 0.0:
        deti = float(LD(1.0) / LD(deti))
    q = np.array(
        [
            [r22 * r33 - r32 * r23, -r12 * r33 + r32 * r13, r12 * r23 - r22 * r13],
            [-r21 * r33 + r31 * r23, r11 * r33 - r31 * r13, -r11 * r23 + r21 * r13],
            [r21 * r32 - r31 * r22, -r11 * r32 + r31 * r12, r11 * r22 - r21 * r12],
        ]
    )
    return np.array([[f32(deti * v) for v in row] for row in q])


def mat33_determ(r: np.ndarray) -> float:
    (r11, r12, r13), (r21, r22, r23), (r31, r32, r33) = (map(float, row) for row in r)
    return f32(
        r11 * r22 * r33
        - r11 * r32 * r23
        - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13
    )


def norm_rows(a: np.ndarray) -> float:
    s = [f32(f32(f32(abs(a[i, 0])) + f32(abs(a[i, 1]))) + f32(abs(a[i, 2]))) for i in range(3)]
    return max(s)


def mat33_polar(a: np.ndarray) -> np.ndarray:
    x = a.copy()
    dif, k = 1.0, 0
    while True:
        y = mat33_inverse(x)
        if dif > 0.3:
            alp = f32(np.sqrt(F32(norm_rows(x)) * F32(norm_rows(x.T))))
            bet = f32(np.sqrt(F32(norm_rows(y)) * F32(norm_rows(y.T))))
            gam = f32(np.sqrt(F32(bet) / F32(alp)))
            gmi = f32(1.0 / gam)
        else:
            gam = gmi = 1.0
        z = np.array(
            [
                [
                    f32(0.5 * float(F32(gam) * F32(x[i, j]) + F32(gmi) * F32(y[j, i])))
                    for j in range(3)
                ]
                for i in range(3)
            ]
        )
        dif = f32(sum(abs(z[i, j] - x[i, j]) for i in range(3) for j in range(3)))
        k += 1
        if k > 100 or dif < 3e-6:
            return z
        x = z


def mat44_to_quatern(m: np.ndarray) -> tuple[float, ...]:
    """(qb, qc, qd, qx, qy, qz) of a single-precision 4 x 4 matrix, the NIfTI-1 reference
    library's way (the polar decomposition of the normalised columns, then the quaternion)."""
    r = [[float(m[i, j]) for j in range(3)] for i in range(3)]
    xd = np.sqrt(r[0][0] ** 2 + r[1][0] ** 2 + r[2][0] ** 2)
    yd = np.sqrt(r[0][1] ** 2 + r[1][1] ** 2 + r[2][1] ** 2)
    zd = np.sqrt(r[0][2] ** 2 + r[1][2] ** 2 + r[2][2] ** 2)
    for i in range(3):
        r[i][0] /= xd
        r[i][1] /= yd
        r[i][2] /= zd
    q = np.array([[f32(v) for v in row] for row in r])
    p = mat33_polar(q)
    r11, r12, r13 = map(float, p[0])
    r21, r22, r23 = map(float, p[1])
    r31, r32, r33 = map(float, p[2])
    zdet = (
        r11 * r22 * r33
        - r11 * r32 * r23
        - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13
    )
    if zdet <= 0:
        r13, r23, r33 = -r13, -r23, -r33
    a = float(LD(r11 + r22 + r33) + LD(1.0))
    if a > 0.5:
        a = float(LD(0.5) * LD(np.sqrt(a)))
        b = float(LD(0.25) * LD(r32 - r23) / LD(a))
        c = float(LD(0.25) * LD(r13 - r31) / LD(a))
        d = float(LD(0.25) * LD(r21 - r12) / LD(a))
    else:
        raise NotImplementedError("the model covers the case a > 0.5 only")
    return b, c, d, float(m[0, 3]), float(m[1, 3]), float(m[2, 3])


# --- the checks ----------------------------------------------------------------------------


def check_inverse(runs: Path) -> tuple[bool, str]:
    a = read_matrix(runs / "inverse-noz" / "noz.mat")
    got = fmt_matrix(inv(a))
    want = (runs / "inverse-noz" / "i.mat").read_text()
    return got == want, got


def check_fixscaleskew(runs: Path) -> tuple[bool, str]:
    d = runs / "fixscaleskew"
    init, fix = read_matrix(d / "init.mat"), read_matrix(d / "s.mat")
    origin = [0.0, 0.0, 0.0]
    pi, pf = decompose(init, origin), decompose(fix, origin)
    got = fmt_matrix(compose(pi[:6] + pf[6:], origin))
    return got == (d / "f.mat").read_text(), got


def check_rotation(runs: Path) -> tuple[bool, str]:
    """The linear part of one composition of 12 parameters (independent of the centre)."""
    d = runs / "rotation"
    p = [0.1, -0.25, 0.3, 0.0, 0.0, 0.0, 1.1, 0.9, 1.05, 0.05, 0.0, 0.02]
    m = compose(p, [0.0, 0.0, 0.0])
    want = [ln.split()[:3] for ln in (d / "out.mat").read_text().splitlines()[:3]]
    got = [[f"{m[r, c]:.10g}" for c in range(3)] for r in range(3)]
    return got == want, f"model {got}\n  FSL {want}"


def header_forms(path: Path):
    import nibabel as nib

    h = nib.load(str(path)).header
    srow = np.array([h["srow_x"], h["srow_y"], h["srow_z"], [0, 0, 0, 1]], dtype=np.float64)
    q = [
        float(h[k])
        for k in ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z")
    ]
    dims = tuple(float(v) for v in h["pixdim"][1:4])
    qmat = quatern_to_mat44(*q, *dims, float(h["pixdim"][0]))
    shape = tuple(int(v) for v in h["dim"][1:4])
    return h, srow, qmat, dims, shape


def check_usesqform(runs: Path) -> tuple[bool, str]:
    d = runs / "usesqform"
    _, s_ref, _, dref, nref = header_forms(d / "t1.nii")
    _, s_in, _, din, _ = header_forms(d / "epi.nii")
    w_ref = swap_lr(s_ref, nref[0], dref)  # the T1w is a neurological file
    m = mul4(np.diag([*dref, 1.0]), inv(w_ref), s_in, inv(np.diag([*din, 1.0])))
    got = fmt_matrix(m)
    return got == (d / "out.mat").read_text(), got


def check_isoxfm(runs: Path) -> tuple[bool, str]:
    d = runs / "isoxfm-oblique"
    _, srow, qmat, dims, shape = header_forms(d / "t1_oblique.nii")
    s1, q1 = swap_lr(srow, shape[0], dims), swap_lr(qmat, shape[0], dims)
    step = [f32(F32(3.5) / F32(v)) for v in dims]
    n_iso = [
        max(1, int(max(1.0, float(F32(n) / F32(st))))) for n, st in zip(shape, step, strict=True)
    ]
    q2 = mul(
        q1, np.diag([*step, 1.0])
    )  # the qform is set (code 1); the sform (code 0) is not composed
    s3, q3 = swap_lr(s1, n_iso[0], (3.5, 3.5, 3.5)), swap_lr(q2, n_iso[0], (3.5, 3.5, 3.5))
    q3f = np.array([[f32(v) for v in row] for row in q3])
    qb, qc, qd, qx, qy, qz = mat44_to_quatern(q3f)
    got_q = [f32(v) for v in (qb, qc, qd, qx, qy, qz)]
    got_s = [f32(v) for v in s3[:3].ravel()]
    h, *_ = header_forms(d / "out.nii.gz")
    want_q = [
        float(F32(h[k]))
        for k in ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z")
    ]
    want_s = [float(v) for v in np.array([h["srow_x"], h["srow_y"], h["srow_z"]]).ravel()]
    ok = got_q == want_q and got_s == want_s and n_iso == [int(v) for v in h["dim"][1:4]]
    detail = f"quatern/qoffset model {got_q}\n  FSL {want_q}\nsrow model {got_s}\n  FSL {want_s}"
    return ok, detail


def main() -> None:
    global VARIANT
    args = sys.argv[1:]
    if "--variant" in args:
        i = args.index("--variant")
        VARIANT = args[i + 1]
        del args[i : i + 2]
    runs = Path(args[0])
    failures = 0
    for name, check in (
        ("inverse-noz (question 11)", check_inverse),
        ("fixscaleskew (question 16)", check_fixscaleskew),
        ("usesqform (question 15)", check_usesqform),
        ("isoxfm-oblique (question 9)", check_isoxfm),
        ("rotation (questions 16 and 6: §3's composition)", check_rotation),
    ):
        ok, detail = check(runs)
        print(f"{name}: {'FSL' if ok else 'DIFFERS'}")
        if not ok:
            failures += 1
            print("  " + detail.replace("\n", "\n  "))
    print(f"{5 - failures} of 5 equal to FSL's output (variant: {VARIANT or 'the spec'})")


if __name__ == "__main__":
    main()
