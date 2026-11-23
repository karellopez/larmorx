# SPDX-License-Identifier: Apache-2.0
"""The probe's voxel hash of `conform` (relay 20, black box).

The probe's README says `conform` prints a volume's voxels as `h` and "the FNV-1a 64 hash of their
bytes". On inputs whose bytes are known (one zero byte, four zero bytes, the int 0, the int 1, the
ints 3 and 1), all five hashes the probe prints come from one start state, 1469598103934665603:
the usual offset basis 14695981039346656037 without its last digit, with the usual prime. This
script runs `conform 2` (a template: the input is printed unchanged) on those inputs and checks
the printed hashes against both start states.

Usage (in a memory box):
    python3 -I conform_hash.py <probe or candidate> <work folder>
"""

import os
import struct
import subprocess
import sys

GEO = (
    "3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 "
    "00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 "
    + "00000000 " * 11
    + "0000000000000000 0 00000000 00000000 0 1 null null"
)
PRIME = 0x100000001B3
STARTS = {"probe's 1469598103934665603": 1469598103934665603, "usual 14695981039346656037": 14695981039346656037}


def fnv1a(b: bytes, h: int) -> int:
    for x in b:
        h = ((h ^ x) * PRIME) % 2**64
    return h


def w(x: int, y: int, z: int, ty: int, fill: str) -> str:
    return f"{x} {y} {z} {ty} 1 0 {fill} {GEO}"


CASES = [
    (w(1, 1, 1, 0, "-1"), b"\0"),
    (w(1, 1, 4, 0, "i0"), bytes(4)),
    (w(1, 1, 1, 1, "i0"), struct.pack("<i", 0)),
    (w(1, 1, 1, 1, "i1"), struct.pack("<i", 1)),
    (w(1, 1, 2, 1, "i3,1"), struct.pack("<2i", 3, 1)),
]


def main() -> None:
    exe, work = sys.argv[1], sys.argv[2]
    os.makedirs(work, exist_ok=True)
    batch = os.path.join(work, "conform_hash.txt")
    with open(batch, "w", encoding="utf-8") as f:
        f.write("".join(f"conform 2 {v} 0 4 3ff0000000000000 0\n" for v, _ in CASES))
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1", "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
    r = subprocess.run([exe, "batch", batch], env=env, cwd=work, capture_output=True, timeout=600, check=False)
    for (_, data), line in zip(CASES, r.stdout.decode("latin-1").rstrip("\n").split("\n"), strict=False):
        printed = line.split("|")[0].split()[-1]
        fits = [name for name, h0 in STARTS.items() if printed == f"h{fnv1a(data, h0):016x}"]
        print(data.hex(), printed, "fits:", ", ".join(fits) or "none")


if __name__ == "__main__":
    main()
