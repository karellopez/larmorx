# SPDX-License-Identifier: Apache-2.0
"""Compares two NIfTI images the way the FLIRT parity suite does: the decompressed bytes with
the header's ``descrip`` (bytes 148-227 of a NIfTI-1 header) blanked, and, when they differ, the
largest data difference and the number of differing voxels.

Usage::

    python -P compare_images.py <a.nii[.gz]> <b.nii[.gz]> [...pairs]
"""

from __future__ import annotations

import gzip
import sys
from pathlib import Path

import numpy as np


def raw(p: Path) -> bytes:
    b = p.read_bytes()
    if b[:2] == b"\x1f\x8b":
        b = gzip.decompress(b)
    return b


def blank_descrip(b: bytes) -> bytes:
    if len(b) >= 348 and int.from_bytes(b[0:4], "little") == 348:
        return b[:148] + b"\0" * 80 + b[228:]
    if len(b) >= 540 and int.from_bytes(b[0:4], "little") == 540:
        return b[:240] + b"\0" * 80 + b[320:]
    return b


def compare(a: Path, b: Path) -> str:
    ra, rb = blank_descrip(raw(a)), blank_descrip(raw(b))
    if ra == rb:
        return f"{a.name} == {b.name}: identical ({len(ra)} bytes)"
    import nibabel as nib

    ia, ib = nib.load(str(a)), nib.load(str(b))
    da = np.asarray(ia.dataobj)
    db = np.asarray(ib.dataobj)
    hdr = "headers differ" if ra[:352] != rb[:352] else "headers equal"
    if da.shape != db.shape:
        return f"{a.name} != {b.name}: {hdr}, shapes {da.shape} {db.shape}"
    diff = da.astype(np.float64) - db.astype(np.float64)
    n = int(np.count_nonzero(diff))
    return (
        f"{a.name} != {b.name}: {hdr}, {n} voxels differ, max |diff| "
        f"{float(np.nanmax(np.abs(diff))) if n else 0.0}"
    )


def main() -> int:
    args = [Path(p) for p in sys.argv[1:]]
    for a, b in zip(args[::2], args[1::2], strict=False):
        print(compare(a, b))
    return 0


if __name__ == "__main__":
    sys.exit(main())
