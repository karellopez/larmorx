# SPDX-License-Identifier: Apache-2.0
"""The spec writer's model of specs/glibc-qsort.md, written from the spec's text (§3 to §5, §10).

Closed to the clean-room implementer of that spec (its author has read glibc's source). Every
function follows one section; the probe's elements and hashes are §10's.
"""

from __future__ import annotations

MASK = (1 << 64) - 1
FNV0 = 14695981039346656037
FNVP = 1099511628211
TAU = 4

FVALS = [0x7FC00000, 0x80000000, 0x00000000, 0x3F800000, 0xBF800000, 0x7F800000, 0xFF800000,
         0x7FC00001, 0xFFC00000, 0x40000000, 0x00000001, 0x3F800000]  # fmt: skip
DVALS = [0x7FF8000000000000, 0x8000000000000000, 0, 0x3FF0000000000000, 0xBFF0000000000000,
         0x7FF0000000000000, 0xFFF0000000000000, 0x7FF8000000000001, 0xFFF8000000000000,
         0x4000000000000000, 1, 0x3FF0000000000000]  # fmt: skip


class OutOfRange(Exception):
    """§5: a position outside 0..n-1 (glibc leaves the array; outside the spec)."""


def _mix(z: int) -> int:
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return z ^ (z >> 31)


class SplitMix:
    """§10's generator."""

    def __init__(self, seed: int):
        self.state = seed & MASK

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        return _mix(self.state)


def elements(s: int, n: int, mode: str, seed: int, keyrange: int) -> list[bytes]:
    """§10: element i of a case."""
    g = SplitMix(seed)
    out = []
    for i in range(n):
        k = g.next() % keyrange
        if mode in ("f3", "f2"):
            out.append(FVALS[k % 12].to_bytes(4, "little"))
            continue
        if mode in ("d3", "d2"):
            out.append(DVALS[k % 12].to_bytes(8, "little"))
            continue
        b = bytearray(s)
        if s == 1:
            b[0] = k % 256
        elif s == 2:
            b[0:2] = (k % 65536).to_bytes(2, "little")
        else:
            b[0:4] = (k % 2**32).to_bytes(4, "little")
            if s >= 8:
                b[4:8] = (i % 2**32).to_bytes(4, "little")
                off = 8
                while off < s:
                    r = g.next().to_bytes(8, "little")
                    m = min(8, s - off)
                    b[off : off + m] = r[:m]
                    off += 8
        out.append(bytes(b))
    return out


def _f32(b: bytes) -> float:
    import struct

    return struct.unpack("<f", b[:4])[0]


def _f64(b: bytes) -> float:
    import struct

    return struct.unpack("<d", b[:8])[0]


def comparator(s: int, mode: str, seed: int):
    """§10's comparator kinds, as a function of two elements' bytes."""
    if s == 1:
        key = lambda b: b[0]  # noqa: E731
    elif s == 2:
        key = lambda b: b[0] | (b[1] << 8)  # noqa: E731
    else:
        key = lambda b: int.from_bytes(b[0:4], "little")  # noqa: E731
    h = (seed ^ 0x5DEECE66D) & MASK
    if mode == "3way":
        return lambda x, y: -1 if key(x) < key(y) else 1 if key(x) > key(y) else 0
    if mode == "tie+":
        return lambda x, y: -1 if key(x) < key(y) else 1
    if mode == "tie-":
        return lambda x, y: 1 if key(x) > key(y) else -1
    if mode == "hash":

        def c(x: bytes, y: bytes) -> int:
            a = int.from_bytes(x[4:8], "little")
            b = int.from_bytes(y[4:8], "little")
            z = h ^ ((a * 0x9E3779B97F4A7C15) & MASK) ^ ((b * 0xC2B2AE3D27D4EB4F) & MASK)
            return _mix(z) % 3 - 1

        return c
    conv = _f32 if mode[0] == "f" else _f64
    if mode[1] == "3":
        return lambda x, y: -1 if conv(x) < conv(y) else 1 if conv(x) > conv(y) else 0
    return lambda x, y: -1 if conv(x) < conv(y) else 0 if conv(x) == conv(y) else 1


class Logged:
    """A comparator that hashes its calls as §10 says."""

    def __init__(self, cmp, s: int):
        self.cmp, self.m, self.calls, self.h = cmp, min(s, 8), 0, FNV0

    def __call__(self, x: bytes, y: bytes) -> int:
        self.calls += 1
        h = self.h
        for byte in x[: self.m] + y[: self.m]:
            h = ((h ^ byte) * FNVP) & MASK
        self.h = h
        return self.cmp(x, y)


def merge_order(a: list[bytes], c) -> None:
    """§4: M(0, n) in place."""

    def m(lo: int, size: int) -> None:
        if size <= 1:
            return
        m1 = size // 2
        m2 = size - m1
        m(lo, m1)
        m(lo + m1, m2)
        left, right = a[lo : lo + m1], a[lo + m1 : lo + size]
        out, i, j = [], 0, 0
        while i < m1 and j < m2:
            if c(left[i], right[j]) <= 0:
                out.append(left[i])
                i += 1
            else:
                out.append(right[j])
                j += 1
        out += left[i:]
        out += right[j:]
        a[lo : lo + size] = out

    m(0, len(a))


def quicksort_order(a: list[bytes], c) -> None:
    """§5: the quicksort phase (n > 4) and the insertion phase; OutOfRange outside the array."""
    n = len(a)

    def at(p: int) -> bytes:
        if p < 0 or p >= n:
            raise OutOfRange(p)
        return a[p]

    def swap(p: int, q: int) -> None:
        at(p), at(q)
        a[p], a[q] = a[q], a[p]

    if n > TAU:
        stack: list[tuple[int, int]] = []
        lo, hi = 0, n - 1
        while True:
            if hi < lo:
                raise OutOfRange((lo, hi))
            mid = lo + (hi - lo) // 2
            if c(at(mid), at(lo)) < 0:
                swap(mid, lo)
            if c(at(hi), at(mid)) < 0:
                swap(mid, hi)
                if c(at(mid), at(lo)) < 0:
                    swap(mid, lo)
            lp, rp = lo + 1, hi - 1
            while True:
                while c(at(lp), at(mid)) < 0:
                    lp += 1
                while c(at(mid), at(rp)) < 0:
                    rp -= 1
                if lp < rp:
                    swap(lp, rp)
                    if mid == lp:
                        mid = rp
                    elif mid == rp:
                        mid = lp
                    lp += 1
                    rp -= 1
                elif lp == rp:
                    lp += 1
                    rp -= 1
                    break
                if lp > rp:
                    break
            left_small = 0 <= rp - lo <= TAU
            right_small = 0 <= hi - lp <= TAU
            if left_small and right_small:
                if not stack:
                    break
                lo, hi = stack.pop()
            elif left_small:
                lo = lp
            elif right_small:
                hi = rp
            elif rp - lo > hi - lp:
                stack.append((lo, rp))
                lo = lp
            else:
                stack.append((lp, hi))
                hi = rp
    if n >= 2:
        t = min(n - 1, TAU)
        k = 0
        for p in range(1, t + 1):
            if c(at(p), at(k)) < 0:
                k = p
        if k != 0:
            swap(k, 0)
        for p in range(2, n):
            q = p - 1
            while c(at(p), at(q)) < 0:
                q -= 1
            q += 1
            if q != p:
                a[q : p + 1] = [a[p], *a[q:p]]


def temp_size(n: int, s: int) -> int:
    """§3's T."""
    return n * s if s <= 32 else 16 * n + s


def run_case(s: int, n: int, mode: str, seed: int, keyrange: int, fail: int):
    """One probe case as the probe prints it: "calls, call hash, sorted hash, requests of T, errno
    kept", or "calls, call hash, outside" when §5 leaves the array (the calls before that point)."""
    a = elements(s, n, mode, seed, keyrange)
    c = Logged(comparator(s, mode, seed), s)
    t = temp_size(n, s)
    requests = 1 if t >= 1024 else 0
    if fail and t >= 1024:
        try:
            quicksort_order(a, c)
        except OutOfRange:
            return f"{c.calls} {c.h:016x} outside"
    else:
        merge_order(a, c)
    h = FNV0
    for e in a:
        for byte in e:
            h = ((h ^ byte) * FNVP) & MASK
    return f"{c.calls} {c.h:016x} {h:016x} {requests} 1"
