# SPDX-License-Identifier: Apache-2.0
"""Probe: nilearn's get_bounds box product and scipy.linalg.inv against candidate exact formulas."""

import numpy as np
import scipy
from scipy import linalg

from larmorx.transforms._linalg import inv as np_inv
from larmorx.transforms._linalg import matmul

rng = np.random.default_rng(0)
nbox2 = 0
nbox = ninv = ninv_np = nprod = 0
N = 3000
for t in range(N):
    # an oblique affine as headers have them (float32 srow values)
    ang = rng.normal(size=3) * 0.3
    from scipy.spatial.transform import Rotation

    R = Rotation.from_rotvec(ang).as_matrix()
    z = rng.choice([1.0, 2.0, 2.5, 3.0, 3.2999990, 0.9375, 1.1], size=3)
    aff = np.eye(4)
    aff[:3, :3] = (R * z).astype(np.float32)
    aff[:3, 3] = (rng.normal(size=3) * 80).astype(np.float32)
    if t % 3 == 0:
        aff[:3, :3] = np.diag(z * rng.choice([-1, 1], size=3)).astype(np.float32)
    shape = tuple(int(v) for v in rng.integers(20, 200, size=3))
    nz = np.round(rng.choice([1.0, 0.8, 1.2, 2.0], size=3).astype(np.float32), 3)
    ta = np.eye(4)
    ta[:3, :3] = np.diag(nz)
    tra = np.linalg.inv(ta).dot(aff)
    # candidate: our numpy inv + matmul
    tra2 = matmul(np_inv(ta), aff)
    nprod += not np.array_equal(tra.view(np.uint64), tra2.view(np.uint64))
    a, b, c = (s - 1 for s in shape)
    box = np.array(
        [
            [0.0, 0, 0, 1],
            [a, 0, 0, 1],
            [0, b, 0, 1],
            [0, 0, c, 1],
            [a, b, 0, 1],
            [a, 0, c, 1],
            [0, b, c, 1],
            [a, b, c, 1],
        ]
    ).T
    ref = np.dot(tra, box)[:3]
    # candidate: sequential FMA chain k = 0..3
    from fractions import Fraction as F

    def fma(x, y, z):
        return float(F(x) * F(y) + F(z))

    cand = np.empty((3, 8))
    cand2 = np.empty((3, 8))
    for i in range(3):
        for j in range(8):
            acc = 0.0
            for k in range(4):
                acc = fma(tra[i, k], box[k, j], acc)
            cand[i, j] = acc
            acc = 0.0
            for k in range(4):
                acc = acc + tra[i, k] * box[k, j]
            cand2[i, j] = acc
    nbox += not np.array_equal(ref.view(np.uint64), cand.view(np.uint64))
    nbox2 += not np.array_equal(ref.view(np.uint64), cand2.view(np.uint64))
    si = linalg.inv(aff)
    ni = np.linalg.inv(aff)
    ninv += not np.array_equal(si.view(np.uint64), ni.view(np.uint64))
    ninv_np += not np.array_equal(ni.view(np.uint64), np_inv(aff).view(np.uint64))
print(
    f"scipy {scipy.__version__} numpy {np.__version__}: {N} cases; inv(ta)@aff vs port {nprod}; box vs fma-chain {nbox} vs plain chain {nbox2}; scipy.inv vs np.inv {ninv}; np.inv vs port {ninv_np}"
)
