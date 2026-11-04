# SPDX-License-Identifier: Apache-2.0
"""Black-box: which arithmetic of fslmaths' ``-edge`` gives FSL's bits on a double image.

``specs/fslmaths.md`` §10.4 writes the squares and ``1/d²`` "by ``pow(x, 2)``". On the suite's
case ``orig/edge-double`` (a 32³ float64 image, uniform in [-20, 20), voxel sizes 2, 2.5 and 3
mm), FSL 6.0.7.23's output (run through the oracle cache, ``use`` mode) is compared, interior
voxel by voxel, with candidate formulas: glibc's ``pow`` (CPython's ``math.pow``, the system's
glibc 2.35, a black box) or exact products, the terms added x, y, z or z, y, x, a division by
``d²`` or a product with ``1/d²``, the denominator rounded to single precision or not.

Result (2026-11-04): only the exact products ``(v₊ − v₋)·(v₊ − v₋) / (d·d)`` added from z to x,
``√Σ / den`` with ``den = (float)(2√(Σ 1/d²))``, give all 27,000 interior voxels; the spec's
order (x, y, z) with glibc's ``pow`` gives 24,352. FSL's ``pow(x, 2)`` is compiled as ``x·x``
(GCC folds it), so ``-edge`` never calls glibc's ``pow``. The clean-room original follows.

Usage: ``python -P edge_forms.py`` (the worktree's package and ``validation/src`` on the path).
"""

from __future__ import annotations

import math
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import fsl_maths as m
from larmorx_validation.parity import fsl_maths_replica as rep

VOX = (2.0, 2.5, 3.0)


def main() -> None:
    run = m._own_cases()["orig/edge-double"][2]
    with tempfile.TemporaryDirectory() as a_:
        a = Path(a_)
        args = m._prepare(run, a)
        rep.run_oracle(args, cwd=a, settings={})
        fsl = np.asarray(nib.load(a / "out.nii.gz").dataobj)
        v = np.asarray(nib.load(a / "dbl32.nii.gz").dataobj).astype(np.float64)
    inv = sum(1.0 / math.pow(x, 2) for x in VOX)
    den = float(np.float32(2.0 * math.sqrt(inv)))
    den_double = 2.0 * math.sqrt(inv)
    forms = {
        "pow, x y z, divide": lambda d: (
            sum3([math.pow(d[a], 2) / math.pow(VOX[a], 2) for a in range(3)], (0, 1, 2)) / den
        ),
        "pow, z y x, divide": lambda d: (
            sum3([math.pow(d[a], 2) / math.pow(VOX[a], 2) for a in range(3)], (2, 1, 0)) / den
        ),
        "pow, x y z, times 1/d²": lambda d: (
            sum3([math.pow(d[a], 2) * (1.0 / math.pow(VOX[a], 2)) for a in range(3)], (0, 1, 2))
            / den
        ),
        "pow of d / voxel, x y z": lambda d: (
            sum3([math.pow(d[a] / VOX[a], 2) for a in range(3)], (0, 1, 2)) / den
        ),
        "products, x y z, divide": lambda d: (
            sum3([d[a] * d[a] / (VOX[a] * VOX[a]) for a in range(3)], (0, 1, 2)) / den
        ),
        "products, z y x, divide": lambda d: (
            sum3([d[a] * d[a] / (VOX[a] * VOX[a]) for a in range(3)], (2, 1, 0)) / den
        ),
        "products, z y x, double denominator": lambda d: (
            sum3([d[a] * d[a] / (VOX[a] * VOX[a]) for a in range(3)], (2, 1, 0)) / den_double
        ),
    }
    agree = dict.fromkeys(forms, 0)
    n = 0
    for i in range(1, 31):
        for j in range(1, 31):
            for k in range(1, 31):
                d = (
                    v[i + 1, j, k] - v[i - 1, j, k],
                    v[i, j + 1, k] - v[i, j - 1, k],
                    v[i, j, k + 1] - v[i, j, k - 1],
                )
                n += 1
                for name, f in forms.items():
                    agree[name] += f(d) == fsl[i, j, k]
    for name, count in agree.items():
        print(f"{name}: {count} of {n} interior voxels FSL's")


def sum3(t: list[float], order: tuple[int, int, int]) -> float:
    """√ of the three terms added in ``order`` from 0."""
    s = 0.0
    for a in order:
        s += t[a]
    return math.sqrt(s)


if __name__ == "__main__":
    main()
