# SPDX-License-Identifier: Apache-2.0
"""Writes two small GE series for FS-680's reproducer (behaviour only: the files' layout).

``<out>/genesis/I.001`` .. ``I.003``: GE Genesis files of 2 x 2 voxels (the magic ``IMGF`` at byte 0,
the pixel data's offset at 4, width and height at 8 and 12, the image header's offset at 148; in the
image header the slice thickness at +26, pixel sizes at +50 and +54, the normal and three corners from
+130, TR/TI/TE at +194/+198/+202, the echoes at +210, the flip angle at +254; big-endian), and
``<out>/lx/e1s1i1`` .. ``e1s1i3``: GE Signa LX files (``IMGF`` at 3,228, width and height at 3,236 and
3,240, voxels from 8,432).

    python -P make_series.py <out>
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path


def put(b: bytearray, at: int, v: bytes) -> None:
    if len(b) < at + len(v):
        b.extend(bytes(at + len(v) - len(b)))
    b[at : at + len(v)] = v


def corners(z: float) -> list[float]:
    # centre, normal, top-left, top-right, bottom-right (R, A, S each): an axial slice
    return [0, 0, z, 0, 0, 1, -1, 1, z, 1, 1, z, 1, -1, z]


def genesis(n: int) -> bytes:
    ih, pixel = 156, 412
    b = bytearray(pixel)
    put(b, 0, b"IMGF")
    put(b, 4, struct.pack(">i", pixel))
    put(b, 8, struct.pack(">ii", 2, 2))
    put(b, 148, struct.pack(">i", ih))
    put(b, ih + 26, struct.pack(">f", 1.0))
    put(b, ih + 50, struct.pack(">ff", 1.0, 1.0))
    put(b, ih + 130, struct.pack(">15f", *corners(float(n))))
    put(b, ih + 194, struct.pack(">iii", 2_000_000, 0, 30_000))
    put(b, ih + 210, struct.pack(">h", 1))
    put(b, ih + 254, struct.pack(">h", 90))
    b += struct.pack(">4h", 10 * n, 10 * n + 1, 10 * n + 2, 10 * n + 3)
    return bytes(b)


def lx(n: int) -> bytes:
    b = bytearray(8432)
    put(b, 3228, b"IMGF")
    put(b, 3236, struct.pack(">ii", 2, 2))
    put(b, 2184 + 28, struct.pack(">f", 1.0))
    put(b, 2184 + 52, struct.pack(">ff", 1.0, 1.0))
    put(b, 2184 + 136, struct.pack(">15f", *corners(float(n))))
    b += struct.pack(">4h", 10 * n, 10 * n + 1, 10 * n + 2, 10 * n + 3)
    return bytes(b)


def main() -> None:
    out = Path(sys.argv[1])
    (out / "genesis").mkdir(parents=True, exist_ok=True)
    (out / "lx").mkdir(parents=True, exist_ok=True)
    for n in (1, 2, 3):
        (out / "genesis" / f"I.{n:03d}").write_bytes(genesis(n))
        (out / "lx" / f"e1s1i{n}").write_bytes(lx(n))


if __name__ == "__main__":
    main()
