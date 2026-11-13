# SPDX-License-Identifier: Apache-2.0
"""specs/glibc-random.md's §2 to §4 in Python, written from the spec's text (the spec writer's model).

Closed to the clean-room implementer of the spec: its writer had read glibc's source (README.md).
"""

from __future__ import annotations

import numpy as np

M = 2147483647  # 2^31 - 1, the seeding congruence's modulus (§2)
A = 16807  # its multiplier
DEG = 31  # the words of the state (§2)
FRONT = 3  # the lag: r_n = r_(n-31) + r_(n-3) (§3)
DISCARD = 310  # values made and dropped by the seeding (§2)
FIRST = 344  # the first delivered value is r_344 (§3)


def seed_words(seed: int) -> list[int]:
    """§2: r_0 ... r_30 for the 32-bit seed (an unsigned integer in [0, 2^32))."""
    s = seed % 2**32
    if s == 0:
        s = 1
    r0 = s - 2**32 if s >= 2**31 else s
    r = [r0]
    for _ in range(1, DEG):
        r.append((A * r[-1]) % M)  # the non-negative residue
    return [v % 2**32 for v in r]


class Stream:
    """One process's state (§3, §4): the values rand() and random() return, in order."""

    def __init__(self, seed: int = 1) -> None:
        self.seed(seed)

    def seed(self, seed: int) -> None:
        r = seed_words(seed)
        # r_31, r_32, r_33 = r_0, r_1, r_2; r_n = r_(n-31) + r_(n-3) mod 2^32 from n = 34
        self.tail = r + r[:3]  # the last terms made, r_(n-len) ... r_(n-1)
        self.n = len(self.tail)  # the index of the next term
        while self.n < FIRST:
            self._next_term()

    def _next_term(self) -> int:
        t = self.tail
        v = (t[-DEG] + t[-FRONT]) % 2**32
        t.append(v)
        if len(t) > 4 * DEG:
            del t[: len(t) - DEG]
        self.n += 1
        return v

    def next(self) -> int:
        """§3: the next value, ⌊r_n / 2⌋ in [0, 2^31 - 1]."""
        return self._next_term() >> 1


def first_values(seeds: np.ndarray, count: int) -> np.ndarray:
    """§2-§3 for many seeds at once: an array (len(seeds), count) of the first values (uint32)."""
    s = seeds.astype(np.uint32).copy()
    s[s == 0] = 1
    word = s.view(np.int32).astype(np.int64)
    ring = np.empty((DEG, s.size), dtype=np.uint32)
    ring[0] = word.astype(np.uint32)
    for i in range(1, DEG):
        word = (A * word) % M  # numpy's % with a positive modulus: the non-negative residue
        ring[i] = word.astype(np.uint32)
    out = np.empty((s.size, count), dtype=np.uint32)
    # r_31..r_33 are r_0..r_2: the same slots of the ring (n mod 31)
    for n in range(34, FIRST + count):
        a = ring[(n - DEG) % DEG]
        b = ring[(n - FRONT) % DEG]
        ring[n % DEG] = a + b  # uint32 wraps modulo 2^32
        if n >= FIRST:
            out[:, n - FIRST] = ring[n % DEG] >> np.uint32(1)
    return out
