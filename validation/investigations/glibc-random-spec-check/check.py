# SPDX-License-Identifier: Apache-2.0
"""Checks specs/glibc-random.md: the spec writer's model against the C library (the open probe).

    python -P check.py <glibc_random_probe> [--exhaustive FIRST LAST]

Without --exhaustive: the default stream, edge and random seeds, the seed conversion, the shared
state, long streams. With --exhaustive: the blocks FIRST to LAST - 1 of 2^24 seeds each (256 blocks
cover every 32-bit seed), the first 4 values of every seed. One line per group: cases, identical,
the first difference.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import model  # noqa: E402

H0 = 14695981039346656037
P = 1099511628211


def fnv(values) -> int:
    h = H0
    for v in values:
        h = ((h ^ v) * P) % 2**64
    return h


def run(probe: str, *args: object) -> list[int]:
    out = subprocess.run([probe, *map(str, args)], capture_output=True, text=True, check=True)
    return [int(x) for x in out.stdout.split()]


def splitmix(state: int):
    while True:
        state = (state + 0x9E3779B97F4A7C15) % 2**64
        z = state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) % 2**64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) % 2**64
        yield z ^ (z >> 31)


def report(name: str, cases: int, same: int, first: str | None) -> bool:
    print(f"{name}: {cases} cases, {same} identical" + (f"; first difference: {first}" if first else ""))
    return same == cases


def quick(probe: str) -> bool:
    ok = True
    # 1. the default stream: as seed 1 (§4)
    ref = run(probe, "default", 100000)
    st = model.Stream(1)
    mine = [st.next() for _ in range(100000)]
    ok &= report("default stream, 100,000 values", 1, int(ref == mine), None if ref == mine else "default")
    # 2. seeds: boundaries of the 32-bit range and of the congruence, FreeSurfer's seeds, random ones,
    # and C's conversion of a long (negative, >= 2^32)
    edge = [0, 1, 2, 3, 53, 1234, 2836, 16807, 127772, 127773, 127774, 2147483646, 2147483647,
            2147483648, 2147483649, 2147483647 + 127773, 4294967294, 4294967295,
            2 * 2147483647, 4294967295 - 127773, 16807 * 127773 % 2**32]
    conv = [-1, -2, -53, -2147483647, -2147483648, -2147483649, 2**32, 2**32 + 5, 2**40 + 7,
            -(2**40) - 7, 2**63 - 1, -(2**63)]
    g = splitmix(0x6C696263)  # 'libc'
    rnd32 = [next(g) % 2**32 for _ in range(150)]
    rnd64 = [next(g) % 2**64 - 2**63 for _ in range(30)]
    seeds = edge + conv + rnd32 + rnd64
    same, first = 0, None
    for s in seeds:
        ref = run(probe, "rand", s, 10000)
        st = model.Stream(s % 2**32)
        mine = [st.next() for _ in range(10000)]
        if ref == mine:
            same += 1
        elif first is None:
            first = f"seed {s}"
    ok &= report("srand + rand, 10,000 values per seed", len(seeds), same, first)
    same, first = 0, None
    sub = edge + conv[:6] + rnd32[:20]
    for s in sub:
        ref = run(probe, "random", s, 3000)
        st = model.Stream(s % 2**32)
        mine = [st.next() for _ in range(3000)]
        if ref == mine:
            same += 1
        elif first is None:
            first = f"seed {s}"
    ok &= report("srandom + random, 3,000 values per seed", len(sub), same, first)
    # 3. one state: rand and random alternate on one stream; srand then srandom (setRandomSeed's
    # two calls) and srandom alone give the stream of srand (§3)
    same, first = 0, None
    sub = [1, 53, 1234, 0, 4294967295] + rnd32[:10]
    for s in sub:
        ref = run(probe, "shared", s, 1000)
        st = model.Stream(s)
        one = [st.next() for _ in range(1000)]
        mine = one + one + one
        if ref == mine:
            same += 1
        elif first is None:
            first = f"seed {s}"
    ok &= report("shared state, 3 x 1,000 values per seed", len(sub), same, first)
    # 4. long streams
    same, first = 0, None
    sub = [1, 53, 1234, 2147483648]
    for s in sub:
        out = subprocess.run([probe, "long", str(s), "1000000"], capture_output=True, text=True)
        ref = int(out.stdout.strip(), 16)
        st = model.Stream(s)
        mine = fnv(st.next() for _ in range(1000000))
        if ref == mine:
            same += 1
        elif first is None:
            first = f"seed {s}"
    ok &= report("long streams, 1,000,000 values per seed (FNV-1a)", len(sub), same, first)
    # 5. the reference hashes quoted in the spec (§6)
    for args in [("hash", 0, 1048576, 8), ("hash", 2147483136, 1024, 64)]:
        out = subprocess.run([probe, *map(str, args)], capture_output=True, text=True)
        seeds_ = (np.arange(args[2], dtype=np.uint64) + args[1]) % 2**32
        vals = model.first_values(seeds_.astype(np.uint32), args[3])
        mine = fnv(int(v) for v in vals.ravel())
        print(f"  {' '.join(map(str, args))}: probe {out.stdout.strip()}, model {mine:016x}")
        ok &= int(out.stdout.strip(), 16) == mine
    return ok


def exhaustive(probe: str, first: int, last: int) -> bool:
    block, chunk, count = 2**24, 2**22, 4
    same_blocks = 0
    for b in range(first, last):
        proc = subprocess.Popen(
            [probe, "raw", str(b * block), str(block), str(count)], stdout=subprocess.PIPE
        )
        good = True
        for c in range(block // chunk):
            data = proc.stdout.read(chunk * count * 4)
            ref = np.frombuffer(data, dtype="<u4").reshape(chunk, count)
            seeds = (np.arange(chunk, dtype=np.uint64) + b * block + c * chunk).astype(np.uint32)
            mine = model.first_values(seeds, count)
            if not np.array_equal(ref, mine):
                bad = np.nonzero((ref != mine).any(axis=1))[0]
                print(f"block {b}: {bad.size} seeds differ, first {int(seeds[bad[0]])}", flush=True)
                good = False
        proc.wait()
        same_blocks += good
        print(f"block {b} ({b * block} to {(b + 1) * block - 1}): {'identical' if good else 'DIFFERS'}", flush=True)
    print(f"exhaustive: blocks {first} to {last - 1}: {same_blocks} of {last - first} identical")
    return same_blocks == last - first


def main(argv: list[str]) -> int:
    probe = argv[0]
    if len(argv) == 4 and argv[1] == "--exhaustive":
        return 0 if exhaustive(probe, int(argv[2]), int(argv[3])) else 1
    return 0 if quick(probe) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
