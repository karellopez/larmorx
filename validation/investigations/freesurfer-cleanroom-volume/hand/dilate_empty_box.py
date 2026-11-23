# SPDX-License-Identifier: Apache-2.0
"""The dilations' box when the search finds nothing (relay 20, black box).

`MRIdilateUchar` and `MRIdilate` (specs/freesurfer-volume.md §48) of an all-zero source into a
given destination of nonzero values: the destination's voxels after the call show which voxels
the box visited (each visited voxel is written 0). Observed: an extent of 1 in all three axes
visits its voxel; widths 2, 3 and 4, a 1 x 4 x 1, a 1 x 1 x 4, a 2 x 2 x 1 and a 3 x 3 x 1 visit
none (the box is not grown when it is empty).

Usage (in a memory box, from any folder):
    python3 -I dilate_empty_box.py <probe or candidate> <work folder> [reproduce|fix]
Prints one line per case: the command and the destination's voxels (the printed result's last
token). The `i` fill writes whole 32-bit words only, so the destinations hold at least 4 bytes
where a written 0 must show.
"""

import os
import subprocess
import sys

GEO = (
    "3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 "
    "00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 "
    + "00000000 " * 11
    + "0000000000000000 0 00000000 00000000 0 1 null null"
)


def w(x: int, y: int, z: int, ty: int, fill: str, nf: int = 1) -> str:
    return f"{x} {y} {z} {ty} {nf} 0 {fill} {GEO}"


ONE_BYTES, ONE_FLOAT, FIVE = "i16843009", "i1065353216", "i5"
CASES = [
    f"dilu {w(1, 1, 1, 0, 'i0')} 0 {w(1, 1, 1, 0, ONE_BYTES, 4)}",
    f"dilu {w(4, 1, 1, 0, 'i0')} 0 {w(4, 1, 1, 0, ONE_BYTES)}",
    f"dilu {w(2, 2, 1, 0, 'i0')} 0 {w(2, 2, 1, 0, ONE_BYTES)}",
    f"dilu {w(1, 4, 1, 0, 'i0')} 0 {w(1, 4, 1, 0, ONE_BYTES)}",
    f"dilu {w(1, 1, 4, 0, 'i0')} 0 {w(1, 1, 4, 0, ONE_BYTES)}",
    f"dilu {w(3, 3, 1, 0, 'i0')} 0 {w(3, 3, 1, 0, ONE_BYTES)}",
    f"dilate {w(1, 1, 1, 3, 'i0')} 0 {w(1, 1, 1, 3, ONE_FLOAT)}",
    f"dilate {w(2, 1, 1, 3, 'i0')} 0 {w(2, 1, 1, 3, ONE_FLOAT)}",
    f"dilate {w(3, 1, 1, 3, 'i0')} 0 {w(3, 1, 1, 3, ONE_FLOAT)}",
    f"dilate {w(4, 1, 1, 3, 'i0')} 0 {w(4, 1, 1, 3, ONE_FLOAT)}",
    f"dilate {w(3, 3, 1, 3, 'i0')} 0 {w(3, 3, 1, 3, ONE_FLOAT)}",
    f"dilate {w(1, 1, 1, 1, 'i0')} 0 {w(1, 1, 1, 1, FIVE)}",
    f"dilate {w(2, 2, 1, 1, 'i0')} 0 {w(2, 2, 1, 1, FIVE)}",
]


def main() -> None:
    exe, work = sys.argv[1], sys.argv[2]
    mode = sys.argv[3] if len(sys.argv) > 3 else "reproduce"
    os.makedirs(work, exist_ok=True)
    batch = os.path.join(work, "dilate_empty_box.txt")
    with open(batch, "w", encoding="utf-8") as f:
        f.write("\n".join(CASES) + "\n")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1", "LARMORX_UPSTREAM_DEFECTS": mode}
    r = subprocess.run([exe, "batch", batch], env=env, cwd=work, capture_output=True, timeout=600, check=False)
    for case, line in zip(CASES, r.stdout.decode("latin-1").rstrip("\n").split("\n"), strict=False):
        print(" ".join(case.split(" ")[:7]), line.split("|")[0].split()[-1])


if __name__ == "__main__":
    main()
