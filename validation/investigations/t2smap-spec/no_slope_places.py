# SPDX-License-Identifier: Apache-2.0
"""numpy's lstsq slope of a column of equal observations (log 256, a saturated uint8 voxel) for
tedana's designs, by its place in the batch (NRHS 1 to 8, each column): which values occur."""

import numpy as np

v = np.float32(np.log(np.float32(256)))
rng = np.random.default_rng(7)
designs = []
for n in (2, 3, 4, 5, 6):
    for tu in (1, 2, 3, 5, 8, 12, 200):
        for _ in range(6):
            tes = np.sort(rng.choice(np.arange(10, 180) / 2, size=n, replace=False))
            designs.append((n, tu, tes))
nd = flips = vary = zero = 0
for n, tu, tes in designs:
    X = np.column_stack([np.ones(n * tu), -np.repeat(tes, tu)])
    vals = set()
    for nrhs in range(1, 9):
        B = np.full((n * tu, nrhs), v, dtype=np.float32)
        b1 = np.linalg.lstsq(X, B, rcond=None)[0][1]
        vals |= set(b1.tolist())
    nd += 1
    s = {np.sign(x) for x in vals}
    vary += len(vals) > 1
    flips += len(s) > 1
    zero += 0.0 in vals
print(
    f"{nd} designs: the slope depends on the column's place in {vary}, its sign in {flips}; exactly 0 in {zero}"
)
