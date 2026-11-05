# SPDX-License-Identifier: Apache-2.0
"""centre_bits.py <bold.nii.gz> [...]: the centres of mass of every volume of a series computed as
the head-motion correction computed them before 2026-11-05 (one double running sum over the
voxels) and as mcflirt does (`specs/mcflirt.md` §6.1: `v − v_min` in single precision, double
running sums flushed into the totals every `max(1000, trunc(√N)) + 1` voxels), and how far apart
they are: how many volumes' centres differ, in how many units in the last place, and in mm.
Volumes as the head-motion correction reads them (float32, x fastest; the x reversal of a
neurologically stored image does not change the sums' order of the y and z coordinates, and the x
coordinate's as mirrored, which this probe leaves out). The clean-room catch-up, relay 7
(2026-11-05)."""

from __future__ import annotations

import sys

import nibabel as nib
import numpy as np


def sums(w: np.ndarray, idx: np.ndarray, block: int | None) -> float:
    """Σ w·idx in double, sequentially (one running sum), or in blocks of ``block`` voxels."""
    t = w * idx
    if block is None:
        return float(np.cumsum(t)[-1])
    n = len(t)
    full = n // block
    total = 0.0
    if full:
        part = np.cumsum(t[: full * block].reshape(full, block), axis=1)[:, -1]
        for p in part:
            total += float(p)
    if n % block:
        total += float(np.cumsum(t[full * block :])[-1])
    return total


def centre(v: np.ndarray, d: np.ndarray, block: int | None) -> np.ndarray:
    nx, ny, nz = v.shape
    flat = np.asarray(v, np.float32).reshape(-1, order="F")
    w = (flat - flat.min()).astype(np.float64)
    i = np.arange(flat.size)
    x, y, z = (i % nx).astype(np.float64), ((i // nx) % ny).astype(np.float64), (i // (nx * ny)).astype(np.float64)
    tw = sums(w, np.ones_like(w), block)
    if abs(tw) < 1e-5:
        tw = 1.0
    return np.array([d[k] * (sums(w, c, block) / tw) for k, c in enumerate((x, y, z))])


def main(argv: list[str]) -> int:
    for path in argv:
        img = nib.load(path)
        data = np.asarray(img.dataobj, dtype=np.float32)
        if data.ndim == 3:
            data = data[..., None]
        d = np.array(img.header.get_zooms()[:3], np.float64)
        n = int(np.prod(data.shape[:3]))
        block = max(1000, int(np.sqrt(n))) + 1
        differ, ulps, mm = 0, [], []
        for t in range(data.shape[3]):
            a = centre(data[..., t], d, None)
            b = centre(data[..., t], d, block)
            if not np.array_equal(a, b):
                differ += 1
                ulps.append(int(np.max(np.abs(a.view(np.int64) - b.view(np.int64)))))
                mm.append(float(np.max(np.abs(a - b))))
        print(
            f"{path}: {data.shape[3]} volumes, blocks of {block} voxels: {differ} centres differ"
            + (f", at most {max(ulps)} ulp, {max(mm):.3g} mm" if differ else "")
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
