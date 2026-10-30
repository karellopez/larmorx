# SPDX-License-Identifier: Apache-2.0
"""How FSL rewrites the qform's quaternion (black-box): for each input, FSL's output quaternion
(``bet2 <input> out -m -n``, read from ``out_mask``) against the NIfTI standard's conversions
quaternion -> matrix -> quaternion in three precisions: the matrix rounded to float, in
double, and the whole computation in x87 extended precision (numpy's ``longdouble``).
``python -P quatern_probe.py <work dir> <input images...>``."""

from __future__ import annotations

import struct
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import fslrun
from header_diff import head


def quat(h: bytes) -> tuple[float, float, float, float]:
    b, c, d = struct.unpack("<3f", h[256:268])
    qfac = struct.unpack("<f", h[76:80])[0]
    return b, c, d, qfac


def to_mat(b, c, d, qfac, dims, T, round_float):
    b, c, d = T(b), T(c), T(d)
    a = T(1) - (b * b + c * c + d * d)
    if a < T(1e-7):
        a = T(1) / np.sqrt(b * b + c * c + d * d)
        b, c, d, a = b * a, c * a, d * a, T(0)
    else:
        a = np.sqrt(a)
    xd, yd, zd = (T(x) if x > 0 else T(1) for x in dims)
    if qfac < 0:
        zd = -zd
    r = [
        [(a * a + b * b - c * c - d * d) * xd, T(2) * (b * c - a * d) * yd, T(2) * (b * d + a * c) * zd],
        [T(2) * (b * c + a * d) * xd, (a * a + c * c - b * b - d * d) * yd, T(2) * (c * d - a * b) * zd],
        [T(2) * (b * d - a * c) * xd, T(2) * (c * d + a * b) * yd, (a * a + d * d - c * c - b * b) * zd],
    ]  # fmt: skip
    if round_float:
        r = [[T(np.float32(v)) for v in row] for row in r]
    return r


def det3(r):
    return (
        r[0, 0] * r[1, 1] * r[2, 2] - r[0, 0] * r[2, 1] * r[1, 2] - r[1, 0] * r[0, 1] * r[2, 2]
        + r[1, 0] * r[2, 1] * r[0, 2] + r[2, 0] * r[0, 1] * r[1, 2] - r[2, 0] * r[1, 1] * r[0, 2]
    )  # fmt: skip


def inv3(r, T):
    (r11, r12, r13), (r21, r22, r23), (r31, r32, r33) = r
    deti = det3(r)
    deti = T(1) / deti if deti != 0 else T(0)
    return np.array(
        [
            [deti * (r22 * r33 - r32 * r23), deti * (-r12 * r33 + r32 * r13), deti * (r12 * r23 - r22 * r13)],
            [deti * (-r21 * r33 + r31 * r23), deti * (r11 * r33 - r31 * r13), deti * (-r11 * r23 + r21 * r13)],
            [deti * (r21 * r32 - r31 * r22), deti * (-r11 * r32 + r31 * r12), deti * (r11 * r22 - r21 * r12)],
        ],
        dtype=T,
    )  # fmt: skip


def polar(m, T):
    x = np.array(m, dtype=T)
    gam = det3(x)
    dif = T(1)
    for _ in range(101):
        y = inv3(x, T)
        if dif > T(0.3):
            alp = np.sqrt(np.abs(x).sum(1).max() * np.abs(x).sum(0).max())
            bet = np.sqrt(np.abs(y).sum(1).max() * np.abs(y).sum(0).max())
            g = np.sqrt(bet / alp)
            gi = T(1) / g
        else:
            g = gi = T(1)
        z = T(0.5) * (g * x + gi * y.T)
        dif = np.abs(z - x).sum()
        x = z
        if dif < T(3e-6):
            break
    _ = gam
    return x


def to_quat(m, T):
    r = np.array(m, dtype=T)
    for j in range(3):
        n = np.sqrt((r[:, j] ** 2).sum())
        r[:, j] /= n if n != 0 else T(1)
    p = polar(r, T)
    if det3(p) <= 0:
        p[:, 2] = -p[:, 2]
    r11, r12, r13 = p[0]
    r21, r22, r23 = p[1]
    r31, r32, r33 = p[2]
    a = r11 + r22 + r33 + T(1)
    if a > T(0.5):
        a = T(0.5) * np.sqrt(a)
        b = T(0.25) * (r32 - r23) / a
        c = T(0.25) * (r13 - r31) / a
        d = T(0.25) * (r21 - r12) / a
    else:
        xd = T(1) + r11 - (r22 + r33)
        yd = T(1) + r22 - (r11 + r33)
        zd = T(1) + r33 - (r11 + r22)
        if xd > T(1):
            b = T(0.5) * np.sqrt(xd)
            c = T(0.25) * (r12 + r21) / b
            d = T(0.25) * (r13 + r31) / b
            a = T(0.25) * (r32 - r23) / b
        elif yd > T(1):
            c = T(0.5) * np.sqrt(yd)
            b = T(0.25) * (r12 + r21) / c
            d = T(0.25) * (r23 + r32) / c
            a = T(0.25) * (r13 - r31) / c
        else:
            d = T(0.5) * np.sqrt(zd)
            b = T(0.25) * (r13 + r31) / d
            c = T(0.25) * (r23 + r32) / d
            a = T(0.25) * (r21 - r12) / d
        if a < 0:
            b, c, d = -b, -c, -d
    return tuple(float(np.float32(v)) for v in (b, c, d))


def main(argv: list[str]) -> int:
    work = Path(argv[0])
    for k, inp in enumerate(argv[1:]):
        d = work / f"q{k}"
        p = fslrun.run("bet2", [inp, "out", "-m", "-n"], d)
        assert p.returncode == 0, p.stderr
        hi, ho = head(Path(inp)), head(d / "out_mask.nii.gz")
        b, c, d_, qfac = quat(hi)
        dims = struct.unpack("<3f", hi[80:92])
        fsl = quat(ho)[:3]
        out = [Path(inp).name, "input", (b, c, d_), "FSL", fsl]
        for name, T, rf in (
            ("float matrix", np.float64, True),
            ("double", np.float64, False),
            ("x87 float matrix", np.longdouble, True),
            ("x87", np.longdouble, False),
            ("float", np.float32, False),
            ("float matrix, double", "f32>f64", False),
            ("double matrix, float", "f64>f32", False),
        ):
            if T == "f32>f64":
                q = to_quat(to_mat(b, c, d_, qfac, dims, np.float32, False), np.float64)
            elif T == "f64>f32":
                q = to_quat(to_mat(b, c, d_, qfac, dims, np.float64, True), np.float32)
            else:
                q = to_quat(to_mat(b, c, d_, qfac, dims, T, rf), T)
            out.append(f"{name}: {'==' if q == fsl else 'differs'}")
        srow = struct.unpack("<12f", hi[280:328])
        scode = struct.unpack("<h", hi[254:256])[0]
        sm = [[float(srow[4 * i + j]) for j in range(3)] for i in range(3)]
        for name, T in (("from sform, double", np.float64), ("from sform, x87", np.longdouble)):
            q = to_quat(sm, T)
            out.append(f"{name} (code {scode}): {'==' if q == fsl else 'differs'}")
        print(*out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
