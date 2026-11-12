# SPDX-License-Identifier: Apache-2.0
"""specs/burkardt-spline.md as a model, written from the spec's text (§3 to §6) by its spec writer,
to check that the spec alone gives FreeSurfer's numbers. CLOSED to the clean-room implementer of the
spec (its author has read Burkardt's and FreeSurfer's source; README.md).

binary32 operations through numpy's float32 scalars, binary64 through Python floats; NaN results
follow x86-64's rule (the first operand's NaN of two) with each operation's operand order explicit.
"""

from __future__ import annotations

import struct

import numpy as np

F = np.float32
NAN32 = struct.unpack("<f", struct.pack("<I", 0xFFC00000))[0]
NAN64 = struct.unpack("<d", struct.pack("<Q", 0xFFF8000000000000))[0]


class SplineError(Exception):
    def __init__(self, code: str, text: str):
        super().__init__(text)
        self.code = code
        self.text = text


def b32(v) -> int:
    return struct.unpack("<I", struct.pack("<f", v))[0]


def f32(bits: int) -> np.float32:
    return F(struct.unpack("<f", struct.pack("<I", bits))[0])


def _nan32(r, a, b):
    if r == r:
        return F(r)
    if a != a:
        return f32(b32(a) | 0x00400000)
    if b != b:
        return f32(b32(b) | 0x00400000)
    return F(NAN32)


def _nan64(r, a, b):
    if r == r:
        return float(r)
    if a != a:
        return struct.unpack(
            "<d",
            struct.pack("<Q", struct.unpack("<Q", struct.pack("<d", a))[0] | 0x0008000000000000),
        )[0]
    if b != b:
        return struct.unpack(
            "<d",
            struct.pack("<Q", struct.unpack("<Q", struct.pack("<d", b))[0] | 0x0008000000000000),
        )[0]
    return NAN64


def _op32(op, a, b):
    a, b = F(a), F(b)
    with np.errstate(all="ignore"):
        r = {"+": a + b, "-": a - b, "*": a * b, "/": a / b}[op]
    return _nan32(r, a, b)


def add32(a, b):
    return _op32("+", a, b)


def sub32(a, b):
    return _op32("-", a, b)


def mul32(a, b):
    return _op32("*", a, b)


def div32(a, b):
    return _op32("/", a, b)


def _op64(op, a, b):
    a, b = float(a), float(b)
    with np.errstate(all="ignore"):
        r = {
            "+": np.float64(a) + b,
            "-": np.float64(a) - b,
            "*": np.float64(a) * b,
            "/": np.float64(a) / b,
        }[op]
    return _nan64(float(r), a, b)


def add64(a, b):
    return _op64("+", a, b)


def sub64(a, b):
    return _op64("-", a, b)


def mul64(a, b):
    return _op64("*", a, b)


def div64(a, b):
    return _op64("/", a, b)


def to32(v: float) -> np.float32:
    if v != v:  # binary64 NaN to binary32: the payload's top bits, quieted
        u = struct.unpack("<Q", struct.pack("<d", v))[0]
        return f32(((u >> 32) & 0x80000000) | 0x7FC00000 | ((u >> 29) & 0x003FFFFF))
    with np.errstate(all="ignore"):
        return F(v)


def _g(v: np.float32) -> str:
    # C's printf("%g") of the value as binary64 (a NaN prints its sign: "-nan")
    if v != v:
        return "-nan" if b32(v) >> 31 else "nan"
    return f"{float(v):g}"


def spline_cubic_set(n, t, y, ibcbeg, ybcbeg, ibcend, ybcend, reproduce=True):
    t = [F(v) for v in t]
    y = [F(v) for v in y]
    ybcbeg, ybcend = F(ybcbeg), F(ybcend)
    head = "\nSPLINE_CUBIC_SET - Fatal error!\n"
    if n <= 1:
        raise SplineError(
            "E1",
            head
            + f"  The number of data points N must be at least 2.\n  The input value is {n}.\n",
        )
    for i in range(n - 1):
        bad = t[i + 1] <= t[i] if reproduce else not (t[i + 1] > t[i])
        if bad:
            raise SplineError(
                "E2",
                head
                + f"  The knots must be strictly increasing, but\n  T({i}) = {_g(t[i])}\n  T({i + 1}) = {_g(t[i + 1])}\n",
            )
    L = [F(0)] * n
    D = [F(0)] * n
    U = [F(0)] * n
    B = [F(0)] * n
    if ibcbeg == 0:
        B[0], D[0], U[0] = F(0), F(1), F(-1)
    elif ibcbeg == 1:
        dt0 = sub32(t[1], t[0])
        B[0] = sub32(div32(sub32(y[1], y[0]), dt0), ybcbeg)
        D[0] = div32(dt0, 3)
        U[0] = div32(dt0, 6)
    elif ibcbeg == 2:
        B[0], D[0], U[0] = ybcbeg, F(1), F(0)
    else:
        raise SplineError(
            "E3", head + f"  IBCBEG must be 0, 1 or 2.\n  The input value is {ibcbeg}.\n"
        )
    for i in range(1, n - 1):
        B[i] = sub32(
            div32(sub32(y[i + 1], y[i]), sub32(t[i + 1], t[i])),
            div32(sub32(y[i], y[i - 1]), sub32(t[i], t[i - 1])),
        )
        L[i] = div32(sub32(t[i], t[i - 1]), 6)
        D[i] = div32(sub32(t[i + 1], t[i - 1]), 3)
        U[i] = div32(sub32(t[i + 1], t[i]), 6)
    if ibcend == 0:
        B[n - 1], L[n - 1], D[n - 1] = F(0), F(-1), F(1)
    elif ibcend == 1:
        dtn = sub32(t[n - 1], t[n - 2])
        B[n - 1] = sub32(ybcend, div32(sub32(y[n - 1], y[n - 2]), dtn))
        L[n - 1] = div32(dtn, 6)
        D[n - 1] = div32(dtn, 3)
    elif ibcend == 2:
        B[n - 1], L[n - 1], D[n - 1] = ybcend, F(0), F(1)
    else:
        raise SplineError(
            "E4", head + f"  IBCEND must be 0, 1 or 2.\n  The input value is {ibcend}.\n"
        )
    if n == 2 and ibcbeg == 0 and ibcend == 0:
        return [F(0), F(0)]
    x = d3_np_fs(n, L, D, U, B)
    if x is None:
        raise SplineError("E5", head + "  The linear system could not be solved.\n")
    return x


def d3_np_fs(n, L, D, U, B):
    if any(d == 0 for d in D):
        return None
    D = list(D)
    x = list(B)
    for i in range(1, n):
        m = div32(L[i], D[i - 1])
        D[i] = sub32(D[i], mul32(U[i - 1], m))  # the binary's order: U·m
        x[i] = sub32(x[i], mul32(m, x[i - 1]))
    x[n - 1] = div32(x[n - 1], D[n - 1])
    for i in range(n - 2, -1, -1):
        x[i] = div32(sub32(x[i], mul32(U[i], x[i + 1])), D[i])
    return x


SENTINEL = 0x7FC00BAD


def spline_cubic_value(n, t, x, y, M):
    """Returns (value, first derivative, second derivative); an unwritten output is SENTINEL's float."""
    t = [F(v) for v in t]
    y = [F(v) for v in y]
    M = [F(v) for v in M]
    x = F(x)
    yp = f32(SENTINEL)
    ypp = f32(SENTINEL)
    if x <= t[0]:
        return y[0], yp, ypp
    if x >= t[n - 1]:
        return y[n - 1], F(0), ypp
    j = n - 2
    for i in range(n - 1):
        if x < t[i + 1]:
            j = i
            break
    dl = sub32(x, t[j])
    h = sub32(t[j + 1], t[j])
    S = div32(sub32(y[j + 1], y[j]), h)
    I = sub32(M[j + 1], M[j])  # noqa: E741
    G = sub64(S, mul64(add64(div64(M[j + 1], 6.0), div64(M[j], 3.0)), h))
    K = div64(I, mul64(6.0, h))
    # the outer operations in the binary's operand order (the nested part first)
    N = mul64(add64(mul64(K, dl), mul64(M[j], 0.5)), dl)
    value = to32(add64(mul64(add64(N, G), dl), y[j]))
    d1 = to32(add64(mul64(add64(mul64(div64(mul64(I, 0.5), h), dl), M[j]), dl), G))
    d2 = add32(div32(mul32(dl, I), h), M[j])
    return value, d1, d2
