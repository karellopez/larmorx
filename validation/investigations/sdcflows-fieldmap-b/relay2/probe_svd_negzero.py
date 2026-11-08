# SPDX-License-Identifier: Apache-2.0
"""Singular 3 x 3 matrices whose numpy.linalg.svd has a -0 singular value (LAPACK 3.12.0)."""

import itertools

import numpy as np

print("numpy", np.__version__)
rng = np.random.default_rng(7)
mats = []
# integer matrices with a zero row, column, or dependent rows
for _ in range(20000):
    m = rng.integers(-3, 4, (3, 3)).astype(float)
    k = rng.integers(0, 4)
    if k == 0:
        m[rng.integers(0, 3)] = 0
    elif k == 1:
        m[:, rng.integers(0, 3)] = 0
    elif k == 2:
        m[2] = m[0] * rng.integers(-2, 3) + m[1] * rng.integers(-2, 3)
    else:
        m[:, 2] = m[:, 0] * rng.integers(-2, 3)
    mats.append(m)
for perm in itertools.permutations(range(3)):
    for signs in itertools.product((1.0, -1.0, 0.0), repeat=3):
        m = np.zeros((3, 3))
        for i, j in enumerate(perm):
            m[i, j] = signs[i]
        mats.append(m)
mats.append(np.zeros((3, 3)))
found = []
for m in mats:
    s = np.linalg.svd(m, compute_uv=True)[1]
    if np.any(np.signbit(s)):
        found.append(m)
print("matrices", len(mats), "with a -0 singular value", len(found))
for m in found[:5]:
    print(m.tolist(), np.linalg.svd(m)[1].tolist())
