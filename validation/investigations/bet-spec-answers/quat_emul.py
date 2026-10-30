# SPDX-License-Identifier: Apache-2.0
"""quat_emul.py <input> <fsl output>...: the qform quaternion the spec's precision path (spec writer,
2026-10-30) gives for an input, against the one FSL wrote (NIfTI-1 floats or NIfTI-2 doubles).
Float steps in numpy float32, double in float64, extended in numpy's longdouble (x87's 80 bits
on x86-64 Linux). Mirroring for positive-determinant inputs is not modelled (radiological inputs)."""

import struct
import sys
from pathlib import Path

import numpy as np

F, D, X = np.float32, np.float64, np.longdouble


def header(p):
    import gzip

    raw = Path(p).read_bytes()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    n = struct.unpack("<i", raw[:4])[0]
    if n == 348:
        pixdim = struct.unpack("<8f", raw[76:108])
        q = struct.unpack("<6f", raw[256:280])
        return 1, pixdim, q
    pixdim = struct.unpack("<8d", raw[104:168])
    q = struct.unpack("<6d", raw[352:400])
    return 2, pixdim, q


def to_mat(qb, qc, qd, pixdim):
    b, c, d = D(F(qb)), D(F(qc)), D(F(qd))
    s = b * b + c * c + d * d
    a = D(X(1) - X(s))
    if X(a) < X("1e-7"):
        a = D(X(1) / X(np.sqrt(s)))
        b, c, d = b * a, c * a, d * a
        a = D(0)
    else:
        a = np.sqrt(a)
    dx, dy, dz, qfac = F(pixdim[1]), F(pixdim[2]), F(pixdim[3]), F(pixdim[0])
    xd = D(dx) if dx > 0 else D(1)
    yd = D(dy) if dy > 0 else D(1)
    zd = D(dz) if dz > 0 else D(1)
    if qfac < 0:
        zd = -zd

    def off(u, sz):  # 2 (u) sz in extended, rounded once to float
        return F(X(2) * X(u) * X(sz))

    R = np.zeros((3, 3), dtype=F)
    R[0, 0] = F((a * a + b * b - c * c - d * d) * xd)
    R[0, 1] = off(b * c - a * d, yd)
    R[0, 2] = off(b * d + a * c, zd)
    R[1, 0] = off(b * c + a * d, xd)
    R[1, 1] = F((a * a + c * c - b * b - d * d) * yd)
    R[1, 2] = off(c * d - a * b, zd)
    R[2, 0] = off(b * d - a * c, xd)
    R[2, 1] = off(c * d + a * b, yd)
    R[2, 2] = F((a * a + d * d - c * c - b * b) * zd)
    return R


def det_d(r):  # double, left to right, from float entries; returned as float
    r11, r12, r13, r21, r22, r23, r31, r32, r33 = (D(v) for v in r.ravel())
    return (
        r11 * r22 * r33
        - r11 * r32 * r23
        - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13
    )


def inv(r):
    r11, r12, r13, r21, r22, r23, r31, r32, r33 = (D(v) for v in r.ravel())
    deti = (
        r11 * r22 * r33
        - r11 * r32 * r23
        - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13
    )
    if deti != 0:
        deti = D(X(1) / X(deti))
    q = [
        deti * (r22 * r33 - r32 * r23),
        deti * (-r12 * r33 + r32 * r13),
        deti * (r12 * r23 - r22 * r13),
        deti * (-r21 * r33 + r31 * r23),
        deti * (r11 * r33 - r31 * r13),
        deti * (-r11 * r23 + r21 * r13),
        deti * (r21 * r32 - r31 * r22),
        deti * (-r11 * r32 + r31 * r12),
        deti * (r11 * r22 - r21 * r12),
    ]
    return np.array(q, dtype=F).reshape(3, 3)


def rownorm(a):
    rs = [F(F(abs(a[i, 0]) + abs(a[i, 1])) + abs(a[i, 2])) for i in range(3)]
    r1 = rs[0]
    if r1 < rs[1]:
        r1 = rs[1]
    if r1 < rs[2]:
        r1 = rs[2]
    return r1


def colnorm(a):
    return rownorm(a.T.copy())


def polar(A):
    Xm = A.copy()
    gam = F(det_d(Xm))
    while gam == 0:
        gam = F(D(0.00001) * (D(0.001) + D(rownorm(Xm))))
        for k in range(3):
            Xm[k, k] = F(Xm[k, k] + gam)
        gam = F(det_d(Xm))
    dif = F(1)
    k = 0
    while True:
        Y = inv(Xm)
        if dif > 0.3:
            alp = np.sqrt(F(rownorm(Xm) * colnorm(Xm)))
            bet = np.sqrt(F(rownorm(Y) * colnorm(Y)))
            gam = np.sqrt(F(bet / alp))
            gmi = F(D(1.0) / D(gam))
        else:
            gam = gmi = F(1)
        Z = np.zeros((3, 3), dtype=F)
        for i in range(3):
            for j in range(3):
                Z[i, j] = F(D(0.5) * D(F(F(gam * Xm[i, j]) + F(gmi * Y[j, i]))))
        dif = F(0)
        for i in range(3):
            for j in range(3):
                dif = F(dif + abs(F(Z[i, j] - Xm[i, j])))
        k += 1
        if k > 100 or dif < 3.0e-6:
            break
        Xm = Z
    return Z


def to_quat(R):
    r = [[D(R[i, j]) for j in range(3)] for i in range(3)]
    for j in range(3):
        n = np.sqrt(r[0][j] * r[0][j] + r[1][j] * r[1][j] + r[2][j] * r[2][j])
        if n == 0:
            for i in range(3):
                r[i][j] = D(1) if i == j else D(0)
            n = D(1)
        for i in range(3):
            r[i][j] = r[i][j] / n
    Q = np.array([[F(r[i][j]) for j in range(3)] for i in range(3)], dtype=F)
    P = polar(Q)
    p = [[D(P[i, j]) for j in range(3)] for i in range(3)]
    r11, r12, r13 = p[0]
    r21, r22, r23 = p[1]
    r31, r32, r33 = p[2]
    zd = (
        r11 * r22 * r33
        - r11 * r32 * r23
        - r21 * r12 * r33
        + r21 * r32 * r13
        + r31 * r12 * r23
        - r31 * r22 * r13
    )
    if zd > 0:
        qfac = 1.0
    else:
        qfac = -1.0
        r13, r23, r33 = -r13, -r23, -r33
    a = D(X(r11 + r22 + r33) + X(1))
    q = lambda num, den: D(X("0.25") * X(num) / X(den))  # noqa: E731
    if a > 0.5:
        a = D(0.5) * np.sqrt(a)
        b, c, d = q(r32 - r23, a), q(r13 - r31, a), q(r21 - r12, a)
    else:
        xd = D(1) + r11 - (r22 + r33)
        yd = D(1) + r22 - (r11 + r33)
        zd = D(1) + r33 - (r11 + r22)
        if xd > 1:
            b = D(0.5) * np.sqrt(xd)
            c, d, a = q(r12 + r21, b), q(r13 + r31, b), q(r32 - r23, b)
        elif yd > 1:
            c = D(0.5) * np.sqrt(yd)
            b, d, a = q(r12 + r21, c), q(r23 + r32, c), q(r13 - r31, c)
        else:
            d = D(0.5) * np.sqrt(zd)
            b, c, a = q(r13 + r31, d), q(r23 + r32, d), q(r21 - r12, d)
        if a < 0:
            b, c, d = -b, -c, -d
    return (b, c, d), qfac


def main():
    _v, pixdim, qin = header(sys.argv[1])
    (b, c, d), _qfac = to_quat(to_mat(qin[0], qin[1], qin[2], pixdim))
    for out in sys.argv[2:]:
        vo, _, qo = header(out)
        mine = (F(b), F(c), F(d)) if vo == 1 else (b, c, d)
        same = [float(m) == float(o) for m, o in zip(mine, qo[:3], strict=True)]
        print(
            f"{out}: NIfTI-{vo} FSL {qo[:3]} spec {tuple(float(m) for m in mine)} {'SAME' if all(same) else 'DIFFER ' + str(same)}"
        )


if __name__ == "__main__":
    main()
