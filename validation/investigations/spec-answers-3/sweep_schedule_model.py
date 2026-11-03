# SPDX-License-Identifier: Apache-2.0
"""The exact parallel order of FAST's in-place sweeps (``specs/fast.md`` §10, spec writer's
answer of 2026-11-03), modelled from the spec text alone: a sweep over the mask's voxels in
storage order (x fastest, then y, then z), each voxel's new value computed from its 26
neighbours as they stand (the ones earlier in the order already updated, the later ones not).

The model runs a toy in-place sweep (every update reads the 3 x 3 x 3 block, the voxel itself
excluded, with distinct weights per offset, so any change in what a voxel reads changes its bits)
three ways on random grids with random masks: serially in storage order; as a pipeline of planes,
plane ``z`` two rows behind plane ``z - 1`` (step ``t = y + 2 z``: every row of one step computed
from the state at the step's start, as concurrent workers would, each row's own voxels in x
order); and with a lag of one row (``t = y + z``) to show that two rows is the least lag. The
first two agree bit for bit on every grid; the lag of one does not.

    python -I validation/investigations/spec-answers-3/sweep_schedule_model.py
"""

from __future__ import annotations

import numpy as np

OFFSETS = [
    (dz, dy, dx)
    for dz in (-1, 0, 1)
    for dy in (-1, 0, 1)
    for dx in (-1, 0, 1)
    if (dz, dy, dx) != (0, 0, 0)
]


def update(read, z: int, y: int, x: int, mask: np.ndarray, w: np.ndarray) -> np.float32:
    s = np.float64(0.0)
    nz, ny, nx = mask.shape
    for k, (dz, dy, dx) in enumerate(OFFSETS):
        zz, yy, xx = z + dz, y + dy, x + dx
        if 0 <= zz < nz and 0 <= yy < ny and 0 <= xx < nx and mask[zz, yy, xx]:
            s += w[k] * np.float64(read(zz, yy, xx))
    return np.float32(np.exp(-abs(s) / 7.0) + 0.25 * np.sin(s))


def serial(v0: np.ndarray, mask: np.ndarray, w: np.ndarray) -> np.ndarray:
    v = v0.copy()
    nz, ny, nx = v.shape
    for z in range(nz):
        for y in range(ny):
            for x in range(nx):
                if mask[z, y, x]:
                    v[z, y, x] = update(lambda a, b, c: v[a, b, c], z, y, x, mask, w)
    return v


def pipelined(v0: np.ndarray, mask: np.ndarray, w: np.ndarray, lag: int) -> np.ndarray:
    v = v0.copy()
    nz, ny, nx = v.shape
    steps: dict[int, list[tuple[int, int]]] = {}
    for z in range(nz):
        for y in range(ny):
            steps.setdefault(y + lag * z, []).append((z, y))
    for t in sorted(steps):
        snap = v.copy()  # what every concurrent row sees of the others
        rows = {}
        for z, y in steps[t]:
            row = snap[z, y].copy()

            def read(a, b, c, z=z, y=y, row=row, snap=snap):
                return row[c] if (a, b) == (z, y) else snap[a, b, c]

            for x in range(nx):
                if mask[z, y, x]:
                    row[x] = update(read, z, y, x, mask, w)
            rows[(z, y)] = row
        for (z, y), row in rows.items():
            v[z, y] = row
    return v


def main() -> None:
    rng = np.random.default_rng(20261103)
    exact = lag1 = 0
    trials = 6
    for _ in range(trials):
        shape = tuple(int(s) for s in rng.integers(4, 9, size=3))
        mask = rng.random(shape) < 0.8
        v0 = rng.random(shape).astype(np.float32)
        w = rng.random(len(OFFSETS)) + 0.5
        a = serial(v0, mask, w)
        b = pipelined(v0, mask, w, lag=2)
        c = pipelined(v0, mask, w, lag=1)
        exact += int(np.array_equal(a, b))
        lag1 += int(np.array_equal(a, c))
        print(
            f"grid {shape}: lag 2 identical {np.array_equal(a, b)}; "
            f"lag 1 identical {np.array_equal(a, c)} ({int(np.sum(a != c))} voxels differ)"
        )
    print(f"lag 2 (pipeline of planes): {exact}/{trials} grids bit-identical to the serial sweep")
    print(f"lag 1: {lag1}/{trials}")


if __name__ == "__main__":
    main()
