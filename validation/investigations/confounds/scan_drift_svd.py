# SPDX-License-Identifier: Apache-2.0
"""numpy's ``svd`` of nipype's cosine drift matrices (``_cosine_drift(period_cut, TR * arange(T))``,
wide when the order exceeds ``T``) and of their transposes against larmorx's
(``_core.numpy_svd``): by default ``T`` 20 to 450 in steps of 7, six cut-off/TR pairs (744 matrices).
The cosine basis has many equal singular values; random testing's finding 54131 (cosine_filter with
T 370, cut-off 5 s, TR 3 s; DBDSQR's tolerance constant, fixed by relay 3a-iv) was one of the two
shapes that differed. ``--t LO HI STEP`` scans another range; ``--lstsq`` also compares
``numpy.linalg.lstsq(drift, b)`` (DGELSD) with ``_core.numpy_lstsq`` for a seeded ``b`` of three
columns. Prints ``K of N identical`` and the shapes that differ.

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python python -P scan_drift_svd.py
    ... python -P scan_drift_svd.py --t 300 470 1 --lstsq
"""

import argparse
import warnings

import numpy as np
from nipype.algorithms.confounds import _cosine_drift

from larmorx import _core

PAIRS = ((5.0, 3.0), (10.0, 2.0), (4.0, 1.0), (30.0, 0.8), (128.0, 2.0), (20.0, 3.0))


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--t", nargs=3, type=int, default=(20, 451, 7), metavar=("LO", "HI", "STEP"))
    p.add_argument("--lstsq", action="store_true")
    args = p.parse_args()
    warnings.simplefilter("ignore")
    lapack = "3.12.0" if np.__version__.startswith(("2.2", "2.3")) else "3.12.1"
    bad, n = [], 0
    rng = np.random.default_rng(54131)
    lo, hi, step = args.t
    for t in range(lo, hi, step):
        for pc, tr in PAIRS:
            drift = _cosine_drift(pc, tr * np.arange(t))
            for m in (drift, drift.T):
                want = np.linalg.svd(m, full_matrices=False)
                got = _core.numpy_svd(np.ascontiguousarray(m), lapack=lapack)
                n += 1
                if not all(np.array_equal(a, b) for a, b in zip(want, got, strict=True)):
                    bad.append(("svd", t, pc, tr, m.shape))
                if args.lstsq:
                    b = rng.integers(-1000, 1000, size=(m.shape[0], 3)) / 64.0
                    want = np.linalg.lstsq(m, b, rcond=None)
                    got = _core.numpy_lstsq(np.ascontiguousarray(m), b, None, lapack=lapack)
                    n += 1
                    same = all(
                        np.array_equal(np.asarray(a), np.asarray(c))
                        for a, c in zip(want, got, strict=True)
                    )
                    if not same:
                        bad.append(("lstsq", t, pc, tr, m.shape))
    print(f"{n - len(bad)} of {n} identical")
    print(bad[:30])
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
