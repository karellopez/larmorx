# SPDX-License-Identifier: Apache-2.0
"""The system C library's ``cbrtf`` as a black box, for ``specs/cbrtf.md``'s validation plan (the
spec writer, 2026-10-30): no C library source is read or needed; the function is called through
``ctypes`` from ``libm.so.6`` (glibc 2.35 on the main machine, Ubuntu 22.04 x86-64).

    python -P validation/investigations/specs-memory/cbrtf_blackbox.py blocks [--jobs 24] [--out f.json]
    python -P validation/investigations/specs-memory/cbrtf_blackbox.py values 0x413f83e6 0x3f800000 ...
    python -P validation/investigations/specs-memory/cbrtf_blackbox.py list 0x413f0000 [--count 65536]

``blocks [first last]``: for each block of 2^20 inputs (4096 blocks; block ``k`` holds the input
bits ``k * 2^20`` to ``(k + 1) * 2^20 - 1``), the hash of its results in input order, the
FNV-1a-like hash larmorx's tools use (``h = 14695981039346656037``; for each result ``h = ((h XOR
bits) * 1099511628211) mod 2^64``, a NaN result counted as ``0x7fc00000``); the block hashes go
into ``--out`` (JSON), and their own hash (the same step over each block hash's low then high 32
bits, in block order) is printed: a candidate is the C library's where every block hash agrees,
and a block that differs is listed with ``list``.
``values``: the result's bits for each input's bits. ``list``: every result of a block, one
``input result`` pair of hexadecimal bits per line.

A NaN input reaches the C function as a ``float`` converted by Python, which quiets a signalling
NaN: NaN results are compared as NaN only (as the hash does).
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.util
import json
import multiprocessing
import struct
from pathlib import Path

import numpy as np

BLOCK = 1 << 20
FNV_OFFSET = 14695981039346656037
FNV_PRIME = 1099511628211
MASK = (1 << 64) - 1


def _cbrtf():
    lib = ctypes.CDLL(ctypes.util.find_library("m") or "libm.so.6")
    f = lib.cbrtf
    f.argtypes = [ctypes.c_float]
    f.restype = ctypes.c_float
    return f


def _bits(y: float) -> int:
    if y != y:
        return 0x7FC00000
    return struct.unpack("<I", struct.pack("<f", y))[0]


def results(start: int, count: int) -> np.ndarray:
    """The result bits of inputs ``start .. start + count - 1``, NaN as ``0x7fc00000``."""
    f = _cbrtf()
    xs = np.arange(start, start + count, dtype=np.uint64).astype(np.uint32).view(np.float32)
    out = np.empty(count, dtype=np.float32)
    for i, x in enumerate(xs.tolist()):
        out[i] = f(x)
    b = out.view(np.uint32).copy()
    b[np.isnan(out)] = 0x7FC00000
    return b


def correctly_rounded(x: float) -> float | None:
    """The float32 nearest to the exact cube root of the float32 ``x`` (ties cannot occur), by
    exact rational arithmetic; ``None`` for 0, infinities and NaN."""
    if x != x or x == 0 or abs(x) == float("inf"):
        return None
    from fractions import Fraction

    ax = Fraction(abs(x))
    y = np.float32(abs(x) ** (1.0 / 3.0))
    # Step to the float32 whose rounding interval holds cbrt(|x|): compare cubes of midpoints.
    for _ in range(8):
        lo = Fraction(float(np.nextafter(y, np.float32(0)))) + Fraction(float(y))
        hi = Fraction(float(y)) + Fraction(float(np.nextafter(y, np.float32(np.inf))))
        if (lo / 2) ** 3 > ax:
            y = np.nextafter(y, np.float32(0))
        elif (hi / 2) ** 3 < ax:
            y = np.nextafter(y, np.float32(np.inf))
        else:
            break
    return float(y) if x > 0 else -float(y)


def block_hash(k: int) -> tuple[int, int]:
    """Block ``k``'s hash, and its results as a chain step: (hash from FNV_OFFSET, block index)."""
    h = FNV_OFFSET
    for b in results(k * BLOCK, BLOCK).tolist():
        h = ((h ^ b) * FNV_PRIME) & MASK
    return k, h


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("what", choices=("blocks", "values", "list"))
    ap.add_argument("args", nargs="*")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--count", type=int, default=65536)
    a = ap.parse_args()
    if a.what == "values":
        f = _cbrtf()
        for v in a.args:
            x = struct.unpack("<f", struct.pack("<I", int(v, 16)))[0]
            y = f(x)
            cr = correctly_rounded(x)
            mark = "" if cr is None or _bits(cr) == _bits(y) else f" (correctly rounded: {_bits(cr):#010x})"
            print(f"{int(v, 16):#010x} {x!r:>16} -> {_bits(y):#010x} {y!r}{mark}")
        return
    if a.what == "list":
        start = int(a.args[0], 16)
        for i, b in enumerate(results(start, a.count).tolist()):
            print(f"{start + i:#010x} {b:#010x}")
        return
    n = (1 << 32) // BLOCK
    first = int(a.args[0], 0) if a.args else 0
    last = int(a.args[1], 0) if len(a.args) > 1 else n
    ctx = multiprocessing.get_context("spawn")
    with ctx.Pool(a.jobs) as pool:
        blocks = dict(pool.imap_unordered(block_hash, range(first, last), chunksize=4))
    # The hash of the block hashes in block order (the same FNV step over their two 32-bit halves,
    # low first): one number for the whole range.
    h = FNV_OFFSET
    for k in sorted(blocks):
        for half in (blocks[k] & 0xFFFFFFFF, blocks[k] >> 32):
            h = ((h ^ half) * FNV_PRIME) & MASK
    print(f"cbrtf: {len(blocks)} blocks of {BLOCK} inputs from block {first}; hash of their hashes {h:016x}")
    if a.out:
        a.out.write_text(
            json.dumps(
                {
                    "hash_of_block_hashes": f"{h:016x}",
                    "block": BLOCK,
                    "blocks": {f"{k:#06x}": f"{v:016x}" for k, v in sorted(blocks.items())},
                }
            ),
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
