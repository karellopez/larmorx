# SPDX-License-Identifier: Apache-2.0
"""crates-nc's golden input ``float32-nonfinite.nii`` (``larmorx-nc-cli/tests/golden.rs``,
``write_inputs``: 16 x 14 x 8 x 4, a NaN in volume 2 and +inf in volume 3), and variants with
other NaNs and infinities, for the NaN matrix (``nan_matrix.py``).

    python -P nan_inputs.py <out.nii> [--nan-bits HEX] [--neg-inf] [--background-nan HEX]
"""

from __future__ import annotations

import struct
import sys

import numpy as np

M = (1 << 64) - 1
SHAPE = (16, 14, 8, 4)
PIXDIM = (3.0, 3.0, 3.5)


def splitmix64(seed: int, n: int) -> list[int]:
    out = []
    for i in range(1, n + 1):
        z = (seed + i * 0x9E3779B97F4A7C15) & M
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M
        out.append(z ^ (z >> 31))
    return out


def head_volume(shape: tuple[int, int, int], shift: list[int], seed: int) -> list[int]:
    nx, ny, nz = shape
    noise = splitmix64(seed, nx * ny * nz)
    out: list[int] = []
    for z in range(nz):
        for y in range(ny):
            for x in range(nx):
                cx = 4 * x - 2 * (nx - 1) - shift[0]
                cy = 4 * y - 2 * (ny - 1) - shift[1]
                cz = 4 * z - 2 * (nz - 1) - shift[2]
                r = 10000 * cx * cx // 576 + 10000 * cy * cy // 484 + 10000 * cz * cz // 144
                v = 600 + (10000 - r) // 25 if r < 10000 else 20
                sx, sy, sz = cx - 8, cy + 6, cz
                if sx * sx + sy * sy + 2 * sz * sz < 64:
                    v += 300
                out.append(v + noise[len(out)] % 41 - 20)
    return out


def main() -> int:
    path = sys.argv[1]
    argv = sys.argv

    def opt(name: str, default: str) -> str:
        return argv[argv.index(name) + 1] if name in argv else default

    nx, ny, nz, nt = SHAPE
    vals: list[int] = []
    for t, s in enumerate([[0, 0, 0], [1, -1, 0], [2, 1, 1], [-1, 2, -1]]):
        vals += head_volume((nx, ny, nz), s, 0x5EED0000 + t)
    b = bytearray((np.array(vals, dtype=np.float32) * np.float32(0.5)).astype("<f4").tobytes())
    per = nx * ny * nz

    def put(i: int, word: bytes) -> None:
        b[4 * i : 4 * i + 4] = word

    put(2 * per + per // 2 + 3, struct.pack("<I", int(opt("--nan-bits", "7fc00000"), 16)))
    put(3 * per + 7, struct.pack("<f", float("inf")))
    if "--neg-inf" in argv:
        put(per + per // 3 + 5, struct.pack("<f", float("-inf")))
    if "--background-nan" in argv:
        bg = struct.pack("<I", int(opt("--background-nan", "7fc00000"), 16))
        arr = np.frombuffer(bytes(b), "<f4")
        for i in np.nonzero(arr < 60)[0][::3]:
            put(int(i), bg)
    h = bytearray(352)
    struct.pack_into("<i", h, 0, 348)
    h[38] = ord("r")
    dims = [nx, ny, nz, nt]
    struct.pack_into("<h", h, 40, len(dims))
    for k in range(7):
        struct.pack_into("<h", h, 42 + 2 * k, dims[k] if k < len(dims) else 1)
    struct.pack_into("<h", h, 70, 16)
    struct.pack_into("<h", h, 72, 32)
    struct.pack_into("<f", h, 76, 1.0)
    for k, d in enumerate(PIXDIM):
        struct.pack_into("<f", h, 80 + 4 * k, d)
    struct.pack_into("<f", h, 92, 2.0)
    struct.pack_into("<f", h, 108, 352.0)
    struct.pack_into("<f", h, 112, 1.0)
    h[123] = 10
    struct.pack_into("<h", h, 254, 1)
    srow = [[-PIXDIM[0], 0, 0, 24.0], [0, PIXDIM[1], 0, -21.0], [0, 0, PIXDIM[2], -14.0]]
    for r, row in enumerate(srow):
        for c, v in enumerate(row):
            struct.pack_into("<f", h, 280 + 16 * r + 4 * c, v)
    h[344:348] = b"n+1\0"
    with open(path, "wb") as f:
        f.write(bytes(h) + bytes(b))
    return 0


if __name__ == "__main__":
    sys.exit(main())
