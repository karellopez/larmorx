# SPDX-License-Identifier: Apache-2.0
"""specs/ranlux.md as a model, written from the spec's text (§2 to §8) by its spec writer, to check
that the spec alone gives FreeSurfer's numbers. CLOSED to the clean-room implementer of the spec
(its author has read GSL and FreeSurfer's transcription; see README.md).

Every binary64 operation is Python's (IEEE double, round to nearest even, SSE2 on x86-64); log, exp
and tan are the C library's through Python's math module (glibc 2.35 on the reference machine);
NaN results follow §7's rule through the helpers below.
"""

from __future__ import annotations

import math
import struct

TWO24 = 1 << 24
PI_D = struct.unpack("<d", (0x400921FB54442D18).to_bytes(8, "little"))[0]
E_D = struct.unpack("<d", (0x4005BF0A8B145769).to_bytes(8, "little"))[0]
DEFAULT_NAN = struct.unpack("<d", (0xFFF8000000000000).to_bytes(8, "little"))[0]


class NeverReturns(Exception):
    """A draw FreeSurfer never returns from (§8.1, §8.2): an error in both modes."""


class InvalidShape(Exception):
    """FS-100's default error (§8.1)."""


def bits(v: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", v))[0]


def from_bits(b: int) -> float:
    return struct.unpack("<d", struct.pack("<Q", b))[0]


def _quiet(v: float) -> float:
    return from_bits(bits(v) | 0x0008000000000000)


def _nan(r: float, a: float, b: float) -> float:
    # §7: one NaN operand is passed on quieted; of two, the first; a NaN made from numbers is the default NaN
    if r == r:
        return r
    if a != a:
        return _quiet(a)
    if b != b:
        return _quiet(b)
    return DEFAULT_NAN


def mul(a: float, b: float) -> float:
    return _nan(a * b, a, b)


def add(a: float, b: float) -> float:
    return _nan(a + b, a, b)


def sub(a: float, b: float) -> float:
    return _nan(a - b, a, b)


def div(a: float, b: float) -> float:
    try:
        r = a / b
    except ZeroDivisionError:
        if a != a:
            return _quiet(a)
        if a == 0.0:
            return DEFAULT_NAN
        return math.copysign(math.inf, a) * math.copysign(1.0, b)
    return _nan(r, a, b)


def neg(a: float) -> float:
    return from_bits(bits(a) ^ 0x8000000000000000)


def c_sqrt(a: float) -> float:
    if a != a:
        return _quiet(a)
    if a < 0:
        return DEFAULT_NAN
    return math.sqrt(a)


def c_log(a: float) -> float:
    if a != a:
        return _quiet(a)
    if a == 0:
        return -math.inf
    if a < 0:
        return DEFAULT_NAN
    if a == math.inf:
        return math.inf
    return math.log(a)


def c_exp(a: float) -> float:
    if a != a:
        return _quiet(a)
    try:
        return math.exp(a)
    except OverflowError:
        return math.inf


def c_tan(a: float) -> float:
    return math.tan(a)


def _quo_rem(t: int, d: int) -> tuple[int, int]:
    q = abs(t) // d
    if t < 0:
        q = -q
    return q, t - d * q


class Generator:
    """§2: the seeded history, the recurrence and the decimation."""

    def __init__(self, seed: int, reproduce: bool = True):
        s = seed % (1 << 64)
        if s == 0:
            s = 314159265
        t = s - (1 << 64) if s >= (1 << 63) else s
        v = []
        for _ in range(24):
            q, r = _quo_rem(t, 53668)
            t = 40014 * r - 12211 * q
            if t < 0:
                t += 2147483563
            v.append(t % TWO24)
        # x_{24-k} = v_k: x[0] = v_24, ..., x[23] = v_1
        self.x = [v[23 - i] for i in range(24)]
        self.c = 0
        self.zero = all(val == 0 for val in self.x)
        if self.zero and not reproduce:
            self.c = 1  # FS-101's correction
            self.zero = False
        self.count = 0

    def _step(self) -> int:
        x = self.x
        d = x[-10] - x[-24] - self.c
        if d < 0:
            d += TWO24
            self.c = 1
        else:
            self.c = 0
        x.append(d)
        del x[0]
        return d

    def next(self) -> int:
        val = self._step()
        self.count += 1
        if self.count == 24:
            self.count = 0
            for _ in range(365):
                self._step()
        return val

    # §3
    def uni(self) -> float:
        return self.next() / 16777216.0

    def uni_pos(self) -> float:
        if self.zero:
            raise NeverReturns("uni_pos")
        while True:
            u = self.uni()
            if u != 0:
                return u


# §4
def flat(g: Generator, a: float, b: float) -> float:
    u = g.uni()
    t1 = mul(u, b)
    t2 = mul(1.0 - u, a)
    return add(t1, t2)


def gaussian(g: Generator, sigma: float) -> float:
    if g.zero:
        raise NeverReturns("gaussian")
    while True:
        x = -1.0 + 2.0 * g.uni()
        y = -1.0 + 2.0 * g.uni()
        r2 = x * x + y * y
        if r2 > 1.0 or r2 == 0.0:
            continue
        t = mul(y, sigma)
        f = c_sqrt((-2.0 * c_log(r2)) / r2)
        return mul(f, t)


def ugauss(g: Generator) -> float:
    return gaussian(g, 1.0)


def exponential(g: Generator, mu: float) -> float:
    p = g.uni_pos()
    return mul(c_log(p), neg(mu))


def g_int(g: Generator, n: int) -> float:
    if n < 12:
        prod = 1.0
        for _ in range(n):
            prod = prod * g.uni_pos()
        return neg(c_log(prod))
    return g_large(g, float(n))


def g_large(g: Generator, a: float) -> float:
    if g.zero:
        raise NeverReturns("gamma_large")
    s = c_sqrt(2.0 * a - 1.0)
    a1 = a - 1.0
    while True:
        while True:
            y = c_tan(PI_D * g.uni())
            sy = mul(s, y)
            t = sub(add(sy, a), 1.0)
            if not (t <= 0):
                break
        v = g.uni()
        rhs = mul(add(1.0, mul(y, y)), c_exp(sub(mul(a1, c_log(div(t, a1))), sy)))
        if v > rhs:
            continue
        return t


def g_frac(g: Generator, f: float) -> float:
    if g.zero:
        raise NeverReturns("gamma_frac")
    if f < 0 or f == -math.inf:
        raise NeverReturns("gamma_frac with a negative shape")
    p = div(E_D, add(f, E_D))
    while True:
        u1 = g.uni()
        v = g.uni_pos()
        if u1 < p:
            x = c_exp(mul(div(1.0, f), c_log(v)))
            q = c_exp(neg(x))
        else:
            x = sub(1.0, c_log(v))
            q = c_exp(mul(sub(f, 1.0), c_log(x)))
        w = g.uni()
        if w >= q:
            continue
        return x


def _n_prime(a: float) -> int:
    # §8.1: the low 32 bits of the 64-bit truncation of floor(a); 0 for NaN, ±inf, |floor(a)| >= 2^63
    if a != a or math.isinf(a):
        return 0
    fl = math.floor(a)
    if fl >= 2**63 or fl < -(2**63):
        return 0
    return int(fl) % (1 << 32)


def gamma(g: Generator, a: float, b: float, reproduce: bool = True) -> float:
    if 0 <= a < 2**32:
        n = math.floor(a)
        if a == n:
            return mul(g_int(g, int(n)), b)
        if n == 0:
            return mul(g_frac(g, a), b)
        gi = g_int(g, int(n))
        return mul(add(gi, g_frac(g, a - n)), b)
    # FS-100
    if a != a or math.isinf(a) or a < 0:
        if a < 0 or a == -math.inf:
            raise NeverReturns("gamma with a negative shape")
        if not reproduce:
            raise InvalidShape(a)
    elif not reproduce:  # a >= 2^32, finite: rule A
        n = math.floor(a)
        if a == n:
            return mul(g_large(g, a), b)
        gi = g_large(g, n)
        return mul(add(gi, g_frac(g, a - n)), b)
    n2 = _n_prime(a)
    if a == float(n2):
        return mul(g_int(g, n2), b)
    if n2 == 0:
        return mul(g_frac(g, a), b)
    gi = g_int(g, n2)
    return mul(add(gi, g_frac(g, a - n2)), b)


def chisq(g: Generator, nu: float) -> float:
    r = gamma(g, nu / 2.0, 1.0)
    return add(r, r)


def fdist(g: Generator, nu1: float, nu2: float) -> float:
    y1 = gamma(g, nu1 / 2.0, 2.0)
    y2 = gamma(g, nu2 / 2.0, 2.0)
    return div(mul(y1, nu2), mul(nu1, y2))


def tdist(g: Generator, nu: float) -> float:
    if nu <= 2:
        y1 = ugauss(g)
        y2 = chisq(g, nu)
        return div(y1, c_sqrt(div(y2, nu)))
    while True:
        y1 = ugauss(g)
        y2 = exponential(g, div(1.0, sub(nu / 2.0, 1.0)))
        z = div(mul(y1, y1), sub(nu, 2.0))
        one_z = sub(1.0, z)
        if one_z < 0:
            continue
        if c_exp(sub(neg(y2), z)) > one_z:
            continue
        return div(y1, c_sqrt(mul(sub(1.0, div(2.0, nu)), one_z)))


DRAWS = {
    "uni": lambda g, p1, p2: g.uni(),
    "uni_pos": lambda g, p1, p2: g.uni_pos(),
    "flat": lambda g, p1, p2: flat(g, p1, p2),
    "gaussian": lambda g, p1, p2: gaussian(g, p1),
    "ugauss": lambda g, p1, p2: ugauss(g),
    "gamma": lambda g, p1, p2: gamma(g, p1, p2),
    "gamma_int": lambda g, p1, p2: g_int(g, int(p1)),
    "gamma_large": lambda g, p1, p2: g_large(g, p1),
    "gamma_frac": lambda g, p1, p2: g_frac(g, p1),
    "chisq": lambda g, p1, p2: chisq(g, p1),
    "fdist": lambda g, p1, p2: fdist(g, p1, p2),
    "tdist": lambda g, p1, p2: tdist(g, p1),
    "exponential": lambda g, p1, p2: exponential(g, p1),
}


def mixed(g: Generator, n: int) -> list[float]:
    """The probe's `mixed` rotation."""
    out = []
    for k in range(n):
        j = k % 14
        if j == 0:
            out.append(g.uni())
        elif j == 1:
            out.append(g.uni_pos())
        elif j == 2:
            out.append(flat(g, -3.5, 7.25))
        elif j == 3:
            out.append(gaussian(g, 2.5))
        elif j == 4:
            out.append(ugauss(g))
        elif j == 5:
            out.append(gamma(g, 3.0, 2.0))
        elif j == 6:
            out.append(gamma(g, 0.4, 1.5))
        elif j == 7:
            out.append(gamma(g, 7.3, 0.5))
        elif j == 8:
            out.append(gamma(g, 25.0, 1.0))
        elif j == 9:
            out.append(gamma(g, 31.6, 1.0))
        elif j == 10:
            out.append(chisq(g, 5.0))
        elif j == 11:
            out.append(fdist(g, 4.0, 9.0))
        elif j == 12:
            out.append(tdist(g, 1.5 if k % 28 == 12 else 7.0))
        else:
            out.append(exponential(g, 0.75))
    return out
