# SPDX-License-Identifier: Apache-2.0
"""nipype's ``regress_poly`` against ``larmorx.confounds.regress_poly`` (``reproduce``) over the
rows' memory order (C and Fortran: numpy's ``pinv(X).dot(data.T)`` takes another BLAS path for
one row of ``pinv(X)``), types, degrees, sizes and ``remove_mean``; then nibabel-like Fortran
arrays of 3 and 4 axes (thin axes make nipype's reshape a view). Prints ``K of N identical``.

Run with the worktree's extension (Haswell kernels, one BLAS thread)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<worktree>/python \\
        python -P probe_regress_layout.py
"""

import itertools
import warnings

import numpy as np
from nipype.algorithms.confounds import regress_poly

import larmorx.confounds as lc


def same(a, b) -> bool:
    return all(
        p.dtype == q.dtype
        and p.shape == q.shape
        and np.ascontiguousarray(p).tobytes() == np.ascontiguousarray(q).tobytes()
        for p, q in zip(a, b, strict=True)
    )


def main() -> int:
    warnings.simplefilter("ignore")
    rng = np.random.default_rng(1)
    sizes = ((387, 69), (40, 10), (5, 300), (1, 69), (69, 1), (2000, 30), (2, 2))
    bad, n = [], 0
    for dt, lay, deg, (v, t), rm in itertools.product(
        ("float32", "float64", "int16"), "CF", (0, 1, 2), sizes, (True, False)
    ):
        x = 100 + 2 * (rng.random((v, t)) + rng.random((v, t)) + rng.random((v, t)) - 1.5)
        x = x.astype(dt)
        x = np.asfortranarray(x) if lay == "F" else x
        want = regress_poly(deg, x, remove_mean=rm)
        got = lc.regress_poly(
            deg, x, remove_mean=rm, numpy_version=np.__version__, upstream_defects="reproduce"
        )
        n += 1
        if not same(want, got):
            bad.append((dt, lay, deg, v, t, rm))
    for shp in ((5, 1, 1, 40), (1, 1, 7, 40), (4, 3, 2, 30), (6, 1, 40), (1, 6, 40), (9, 40)):
        for deg in (0, 1):
            x = np.asfortranarray(100 + rng.random(shp))
            want = regress_poly(deg, x)
            got = lc.regress_poly(
                deg, x, numpy_version=np.__version__, upstream_defects="reproduce"
            )
            n += 1
            if not same(want, got):
                bad.append(("nibabel-like", shp, deg))
    for b in bad[:40]:
        print("differ", b)
    print(f"{n - len(bad)} of {n} identical")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
